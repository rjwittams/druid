use crate::widget::Augmented;
use crate::{Data, Env, Widget, WidgetExt, WidgetPod};
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Add, Deref, DerefMut};

/// Content - a possibly dynamic list of widget pods.
/// The widgets within those pods are ensured to have a particular augmentation available
pub trait Content<T, Aug> {
    /// If possible, add this child widget to the content.
    fn add_child_widget(&mut self, cw: EnsuredPod<T, Aug>) -> bool;
    /// Content initially created
    fn content_added(&mut self, data: &T, env: &Env);
    /// Data changed - return value indicates if the contained child widgets changed.
    fn update(&mut self, old_data: &T, data: &T, env: &Env) -> bool;
    /// Get a mutable ref to the child at idx
    fn child_mut(&mut self, idx: usize) -> Option<&mut EnsuredPod<T, Aug>>;
    /// Get an immutable ref to the last child
    fn last_child(&self) -> Option<&EnsuredPod<T, Aug>>;
    /// Number of children available
    fn len(&self) -> usize;
    /// Is the content empty of widgets
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T, Aug> Content<T, Aug> for Box<dyn Content<T, Aug>> {
    fn add_child_widget(&mut self, cw: EnsuredPod<T, Aug>) -> bool {
        self.deref_mut().add_child_widget(cw)
    }

    fn content_added(&mut self, data: &T, env: &Env) {
        self.deref_mut().content_added(data, env)
    }

    fn update(&mut self, old_data: &T, data: &T, env: &Env) -> bool {
        self.deref_mut().update(old_data, data, env)
    }

    fn child_mut(&mut self, idx: usize) -> Option<&mut EnsuredPod<T, Aug>> {
        self.deref_mut().child_mut(idx)
    }

    fn last_child(&self) -> Option<&EnsuredPod<T, Aug>> {
        self.deref().last_child()
    }

    fn len(&self) -> usize {
        self.deref().len()
    }
}

/// Extension methods for Content that are not object safe
pub trait ContentExt<T, Aug>: Content<T, Aug> {
    /// Do something for each pod
    fn for_each_child(&mut self, mut f: impl FnMut(&mut EnsuredPod<T, Aug>)) {
        for idx in 0..self.len() {
            if let Some(child) = self.child_mut(idx) {
                f(child)
            }
        }
    }

    /// Compose this content with another
    fn then<Other: Content<T, Aug>>(self, other: Other) -> ComposedContent<T, Self, Other>
    where
        Self: Sized,
    {
        ComposedContent::new(self, other)
    }
}

impl<T, Aug, F: Content<T, Aug>> ContentExt<T, Aug> for F {}

impl<T, Aug, Content2: Content<T, Aug>> Add<Content2> for StaticContent<T, Aug> {
    type Output = ComposedContent<T, StaticContent<T, Aug>, Content2>;

    fn add(self, rhs: Content2) -> Self::Output {
        ComposedContent::new(self, rhs)
    }
}

impl<T: Data, K, Aug, Content2: Content<T, Aug>> Add<Content2> for ForEachContent<T, K, Aug> {
    type Output = ComposedContent<T, Self, Content2>;

    fn add(self, rhs: Content2) -> Self::Output {
        ComposedContent::new(self, rhs)
    }
}

impl<T: Data, C1Self, C2Self, Content2> Add<Content2> for ComposedContent<T, C1Self, C2Self> {
    type Output = ComposedContent<T, Self, Content2>;

    fn add(self, rhs: Content2) -> Self::Output {
        ComposedContent::new(self, rhs)
    }
}

impl<T, ContentTrue, ContentFalse, Content2> Add<Content2>
    for ConditionalContent<T, ContentTrue, ContentFalse>
{
    type Output = ComposedContent<T, Self, Content2>;

    fn add(self, rhs: Content2) -> Self::Output {
        ComposedContent::new(self, rhs)
    }
}

/// Content that does not update when the data changes
pub struct StaticContent<T, Aug> {
    children: Vec<EnsuredPod<T, Aug>>,
}

impl<T, Aug> Default for StaticContent<T, Aug> {
    fn default() -> Self {
        StaticContent {
            children: Default::default(),
        }
    }
}

impl<T: Data, Aug: Default + Clone + 'static> StaticContent<T, Aug> {
    /// Content consisting of one widget
    pub fn of(w: impl Widget<T> + 'static) -> Self {
        Self::default().with_child(w)
    }

    /// This content with an additional widget
    pub fn with_child(mut self, w: impl Widget<T> + 'static) -> Self {
        self.add_child_widget(EnsuredPod::new(w));
        self
    }
}

impl<T: Data, Aug> Content<T, Aug> for StaticContent<T, Aug> {
    fn add_child_widget(&mut self, cw: EnsuredPod<T, Aug>) -> bool {
        self.children.push(cw);
        true
    }

    fn content_added(&mut self, _data: &T, _env: &Env) {}

    fn update(&mut self, _old_data: &T, _data: &T, _env: &Env) -> bool {
        false
    }

    fn child_mut(&mut self, idx: usize) -> Option<&mut EnsuredPod<T, Aug>> {
        self.children.get_mut(idx)
    }

    fn last_child(&self) -> Option<&EnsuredPod<T, Aug>> {
        self.children.last()
    }

    fn len(&self) -> usize {
        self.children.len()
    }
}

type ValuesFromData<T, K> = dyn Fn(&T, &Env) -> Vec<K>;
type WidgetFromValue<T, K, Aug> = dyn Fn(&T, &Env, K) -> EnsuredPod<T, Aug>;

/// Content that is derived from data
///
/// This is currently very basic and holds the 'indices' of widgets in a vec,
/// and rederives that vec whenever data changes.
///
/// It also doesn't do 'stable keys' for items moving around yet, so widgets will get reused for indexes
/// (that might not correspond to their logical identity, so widget internal state could be mismatched if derived at instantiation).
/// Currently all widget indexes that have ever existed will have a widget that might not be shown.
/// Need a lifetime policy of some kind e.g instant drop, keep alive, keep forever.
///
/// Needs to support diffable collections, and range iteration of some kind (for virtualised lists of large objects).
/// Intersection of these maybe slightly involved.
///
pub struct ForEachContent<T, K, Aug: 'static> {
    values_from_data: Box<ValuesFromData<T, K>>,
    make_widget: Box<WidgetFromValue<T, K, Aug>>,
    values: Vec<K>,
    child_widgets: HashMap<K, EnsuredPod<T, Aug>>,
}

impl<T: Data, K, Aug: Default + Clone + 'static> ForEachContent<T, K, Aug> {
    /// Create a dynamic list of widgets
    pub fn new<I: IntoIterator<Item = K>, W: Widget<T> + 'static>(
        values_from_data: impl Fn(&T, &Env) -> I + 'static,
        make_widget: impl Fn(&T, &Env, K) -> W + 'static,
    ) -> Self {
        ForEachContent {
            values_from_data: Box::new(move |data, env| {
                values_from_data(data, env).into_iter().collect()
            }),
            make_widget: Box::new(move |data, env, key| {
                let widget = make_widget(data, env, key);
                EnsuredPod::new(widget)
            }),
            values: Default::default(),
            child_widgets: Default::default(),
        }
    }
}

impl<T: Data, K: Hash + Eq + Clone, Aug> ForEachContent<T, K, Aug> {
    fn update_impl(&mut self, data: &T, env: &Env) -> bool {
        let mut new_values = (*self.values_from_data)(data, env);

        let make_widget = &self.make_widget;
        for value in &new_values {
            self.child_widgets
                .entry(value.clone())
                .or_insert_with(|| (*make_widget)(data, env, value.clone()));
        }
        std::mem::swap(&mut new_values, &mut self.values);
        new_values == self.values
    }
}

impl<T: Data, K: Hash + Eq + Clone, Aug> Content<T, Aug> for ForEachContent<T, K, Aug> {
    fn content_added(&mut self, data: &T, env: &Env) {
        self.update_impl(data, env);
    }

    fn update(&mut self, old_data: &T, data: &T, env: &Env) -> bool {
        if !old_data.same(data) {
            self.update_impl(data, env)
        } else {
            false
        }
    }

    fn add_child_widget(&mut self, _cw: EnsuredPod<T, Aug>) -> bool {
        false
    }

    fn child_mut(&mut self, idx: usize) -> Option<&mut EnsuredPod<T, Aug>> {
        let child_widgets = &mut self.child_widgets;
        if let Some(val) = self.values.get(idx) {
            child_widgets.get_mut(val)
        } else {
            None
        }
    }

    fn last_child(&self) -> Option<&EnsuredPod<T, Aug>> {
        self.values
            .last()
            .and_then(|val| self.child_widgets.get(val))
    }

    fn len(&self) -> usize {
        self.values.len()
    }
}

pub struct ComposedContent<T, Content1, Content2> {
    phantom_t: PhantomData<T>,
    content1: Content1,
    content2: Content2,
}

impl<T, Content1, Content2> ComposedContent<T, Content1, Content2> {
    pub fn new(content1: Content1, content2: Content2) -> Self {
        ComposedContent {
            phantom_t: Default::default(),
            content1,
            content2,
        }
    }
}

impl<T, Aug, Content1: Content<T, Aug>, Content2: Content<T, Aug>> Content<T, Aug>
    for ComposedContent<T, Content1, Content2>
{
    fn content_added(&mut self, data: &T, env: &Env) {
        self.content1.content_added(data, env);
        self.content2.content_added(data, env);
    }

    fn update(&mut self, old_data: &T, data: &T, env: &Env) -> bool {
        let up1 = self.content1.update(old_data, data, env);
        let up2 = self.content2.update(old_data, data, env);
        up1 || up2
    }

    fn add_child_widget(&mut self, cw: EnsuredPod<T, Aug>) -> bool {
        self.content2.add_child_widget(cw)
    }

    fn child_mut(&mut self, idx: usize) -> Option<&mut EnsuredPod<T, Aug>> {
        let len1 = self.content1.len();
        if idx < len1 {
            self.content1.child_mut(idx)
        } else {
            self.content2.child_mut(idx - len1)
        }
    }

    fn last_child(&self) -> Option<&EnsuredPod<T, Aug>> {
        self.content2
            .last_child()
            .or_else(|| self.content1.last_child())
    }

    fn len(&self) -> usize {
        self.content1.len() + self.content2.len()
    }
}

type Predicate<T> = dyn Fn(&T, &Env) -> bool;

struct CondBranch<T, C> {
    und: C,
    shown: bool,
    phantom_t: PhantomData<T>,
}

impl<T, C> CondBranch<T, C> {
    fn new(und: C) -> Self {
        Self {
            und,
            shown: false,
            phantom_t: PhantomData,
        }
    }
}

impl<T, C> CondBranch<T, C> {
    fn content_added<Aug>(&mut self, data: &T, env: &Env)
    where
        C: Content<T, Aug>,
    {
        self.und.content_added(data, env);
        self.shown = true
    }

    fn update<Aug>(&mut self, old_data: &T, data: &T, env: &Env) -> bool
    where
        C: Content<T, Aug>,
    {
        if self.shown {
            self.und.update(old_data, data, env)
        } else {
            self.und.content_added(data, env);
            self.shown = true;
            true
        }
    }
}

/// Content that may have one of two underlying parts shown.
pub struct ConditionalContent<T, ContentTrue, ContentFalse> {
    condition: Box<Predicate<T>>,
    true_br: CondBranch<T, ContentTrue>,
    false_br: CondBranch<T, ContentFalse>,
    current: Option<bool>,
}

impl<T: Data, Aug, ContentTrue> ConditionalContent<T, ContentTrue, StaticContent<T, Aug>> {
    /// Create content that is shown if a predicate is true
    pub fn new_if(cond: impl Fn(&T, &Env) -> bool + 'static, content_true: ContentTrue) -> Self {
        ConditionalContent {
            condition: Box::new(cond),
            true_br: CondBranch::new(content_true),
            false_br: CondBranch::new(Default::default()),
            current: None,
        }
    }
}

impl<T: Data, ContentTrue, ContentFalse> ConditionalContent<T, ContentTrue, ContentFalse> {
    /// Create content with two branches, which are switched between based on a predicate
    pub fn new_if_else(
        cond: impl Fn(&T, &Env) -> bool + 'static,
        content_true: ContentTrue,
        content_false: ContentFalse,
    ) -> Self {
        ConditionalContent {
            condition: Box::new(cond),
            true_br: CondBranch::new(content_true),
            false_br: CondBranch::new(content_false),
            current: None,
        }
    }
}

impl<T: Data, Aug, ContentTrue: Content<T, Aug>, ContentFalse: Content<T, Aug>> Content<T, Aug>
    for ConditionalContent<T, ContentTrue, ContentFalse>
{
    fn add_child_widget(&mut self, _cw: EnsuredPod<T, Aug>) -> bool {
        false
    }

    fn content_added(&mut self, data: &T, env: &Env) {
        let cur = (*self.condition)(data, env);
        self.current = Some(cur);
        if cur {
            self.true_br.content_added(data, env)
        } else {
            self.false_br.content_added(data, env)
        }
    }

    fn update(&mut self, old_data: &T, data: &T, env: &Env) -> bool {
        let cond_changed = if !old_data.same(data) {
            let new_cond = Some((*self.condition)(data, env));
            let changed = self.current == new_cond;
            self.current = new_cond;
            changed
        } else {
            false
        };

        let und_changed = if let Some(cond) = self.current {
            if cond {
                self.true_br.update(old_data, data, env)
            } else {
                self.false_br.update(old_data, data, env)
            }
        } else {
            false
        };

        cond_changed || und_changed
    }

    fn child_mut(&mut self, idx: usize) -> Option<&mut EnsuredPod<T, Aug>> {
        if let Some(cond) = self.current {
            if cond {
                self.true_br.und.child_mut(idx)
            } else {
                self.false_br.und.child_mut(idx)
            }
        } else {
            None
        }
    }

    fn last_child(&self) -> Option<&EnsuredPod<T, Aug>> {
        if let Some(cond) = self.current {
            if cond {
                self.true_br.und.last_child()
            } else {
                self.false_br.und.last_child()
            }
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        if let Some(cond) = self.current {
            if cond {
                self.true_br.und.len()
            } else {
                self.false_br.und.len()
            }
        } else {
            0
        }
    }
}

pub struct EnsuredPod<T, Aug> {
    pub(crate) pod: WidgetPod<T, Box<dyn Widget<T>>>,
    phantom_a: PhantomData<Aug>,
}

impl<T: Data, Aug: Default + Clone + 'static> EnsuredPod<T, Aug> {
    pub(crate) fn new(child: impl Widget<T> + 'static) -> Self {
        if child.augmentation::<Aug>().is_some() {
            EnsuredPod {
                pod: WidgetPod::new(Box::new(child)),
                phantom_a: Default::default(),
            }
        } else {
            let aug: Aug = Default::default();
            EnsuredPod {
                pod: WidgetPod::new(Box::new(Augmented::new(child, aug))),
                phantom_a: Default::default(),
            }
        }
    }
}

impl<T: Data, Aug: 'static> Deref for EnsuredPod<T, Aug> {
    type Target = WidgetPod<T, Box<dyn Widget<T>>>;

    fn deref(&self) -> &Self::Target {
        &self.pod
    }
}

impl<T: Data, Aug: 'static> DerefMut for EnsuredPod<T, Aug> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.pod
    }
}

impl<T: Data, Aug: 'static> EnsuredPod<T, Aug> {
    pub fn aug(&self) -> &Aug {
        self.widget().augmentation().unwrap()
    }
}
