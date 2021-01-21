use crate::{Env, Data, Widget, WidgetExt, WidgetPod};
use std::ops::{DerefMut, Deref};
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

pub trait Content<T, Aug> {
    fn content_added(&mut self, data: &T, env: &Env);
    fn update(&mut self, old_data: &T, data: &T, env: &Env);
    fn add_child_widget(&mut self, cw: ChildWidget<T, Aug>) -> bool;
    fn child_mut(&mut self, idx: usize) -> Option<&mut ChildWidget<T, Aug>>;
    fn last_child(&self) -> Option<&ChildWidget<T, Aug>>;
    fn len(&self) -> usize;
}

impl<T, Aug> Content<T, Aug> for Box<dyn Content<T, Aug>> {
    fn content_added(&mut self, data: &T, env: &Env) {
        self.deref_mut().content_added(data, env)
    }

    fn update(&mut self, old_data: &T, data: &T, env: &Env) {
        self.deref_mut().update(old_data, data, env)
    }

    fn add_child_widget(&mut self, cw: ChildWidget<T, Aug>) -> bool {
        self.deref_mut().add_child_widget(cw)
    }

    fn child_mut(&mut self, idx: usize) -> Option<&mut ChildWidget<T, Aug>> {
        self.deref_mut().child_mut(idx)
    }

    fn last_child(&self) -> Option<&ChildWidget<T, Aug>> {
        self.deref().last_child()
    }

    fn len(&self) -> usize {
        self.deref().len()
    }
}

pub trait ContentExt<T, Aug>: Content<T, Aug> {
    fn for_each_child(&mut self, mut f: impl FnMut(&mut ChildWidget<T, Aug>)) {
        for idx in 0..self.len() {
            if let Some(child) = self.child_mut(idx) {
                f(child)
            }
        }
    }

    fn then<Other: Content<T, Aug>>(self, other: Other) -> ComposedContent<T, Self, Other>
        where
            Self: Sized,
    {
        ComposedContent {
            phantom_t: Default::default(),
            content1: self,
            content2: other,
        }
    }
}

impl<T, Aug, F: Content<T, Aug>> ContentExt<T, Aug> for F {}

pub struct StaticContent<T, Aug> {
    children: Vec<ChildWidget<T, Aug>>,
}

impl<T, Aug> StaticContent<T, Aug> {
    pub fn new() -> Self {
        StaticContent {
            children: Default::default(),
        }
    }
}

impl<T: Data, Aug: Default + Clone + 'static> StaticContent<T, Aug> {
    pub fn of(w: impl Widget<T> + 'static) -> Self {
        Self::new().with_child(w)
    }

    pub fn with_child(mut self, w: impl Widget<T> + 'static) -> Self {
        self.add_child_widget(ChildWidget::new(w));
        self
    }
}

impl<T: Data, Aug> Content<T, Aug> for StaticContent<T, Aug> {
    fn content_added(&mut self, _data: &T, _env: &Env) {}

    fn update(&mut self, _old_data: &T, _data: &T, _env: &Env) {}

    fn add_child_widget(&mut self, cw: ChildWidget<T, Aug>) -> bool {
        self.children.push(cw);
        true
    }

    fn child_mut(&mut self, idx: usize) -> Option<&mut ChildWidget<T, Aug>> {
        self.children.get_mut(idx)
    }

    fn last_child(&self) -> Option<&ChildWidget<T, Aug>> {
        self.children.last()
    }

    fn len(&self) -> usize {
        self.children.len()
    }
}

type ValuesFromData<T, K> = dyn Fn(&T, &Env) -> Vec<K>;
type WidgetFromValue<T, K, Aug> = dyn Fn(&T, &Env, &K) -> ChildWidget<T, Aug>;

pub struct ForEachContent<T, K, Aug> {
    values_from_data: Box<ValuesFromData<T, K>>,
    make_widget: Box<WidgetFromValue<T, K, Aug>>,
    values: Vec<K>,
    child_widgets: HashMap<K, ChildWidget<T, Aug>>,
}

impl<T: Data, K, Aug: Default + Clone + 'static> ForEachContent<T, K, Aug> {
    pub fn new<I: IntoIterator<Item = K>, W: Widget<T> + 'static>(
        values_from_data: impl Fn(&T, &Env) -> I + 'static,
        make_widget: impl Fn(&T, &Env, &K) -> W + 'static,
    ) -> Self {
        ForEachContent {
            values_from_data: Box::new(move |data, env| {
                values_from_data(data, env).into_iter().collect()
            }),
            make_widget: Box::new(move |data, env, key| {
                let widget = make_widget(data, env, key);
                ChildWidget::new(widget)
            }),
            values: Default::default(),
            child_widgets: Default::default()
        }
    }
}

impl<T: Data, K: Hash + Eq + Clone, Aug> Content<T, Aug> for ForEachContent<T, K, Aug> {
    fn content_added(&mut self, data: &T, env: &Env) {
        self.values = (*self.values_from_data)(data, env);
        let make_widget = &self.make_widget;
        for value in &self.values {
            self.child_widgets.entry(value.clone()).or_insert_with(|| {
                (*make_widget)(data, env, value)
            });
        }
    }

    fn update(&mut self, old_data: &T, data: &T, env: &Env) {
        if !old_data.same(data) {
            self.content_added(data, env)
        }
    }

    fn add_child_widget(&mut self, _cw: ChildWidget<T, Aug>) -> bool {
        false
    }

    fn child_mut(&mut self, idx: usize) -> Option<&mut ChildWidget<T, Aug>> {
        let child_widgets = &mut self.child_widgets;
        if let Some(val) = self.values.get(idx) {
            child_widgets.get_mut(val)
        } else {
            None
        }
    }

    fn last_child(&self) -> Option<&ChildWidget<T, Aug>> {
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

impl<T, Aug, Content1: Content<T, Aug>, Content2: Content<T, Aug>> Content<T, Aug>
for ComposedContent<T, Content1, Content2>
{
    fn content_added(&mut self, data: &T, env: &Env) {
        self.content1.content_added(data, env);
        self.content2.content_added(data, env);
    }

    fn update(&mut self, old_data: &T, data: &T, env: &Env) {
        self.content1.update(old_data, data, env);
        self.content2.update(old_data, data, env);
    }

    fn add_child_widget(&mut self, cw: ChildWidget<T, Aug>) -> bool {
        self.content2.add_child_widget(cw)
    }

    fn child_mut(&mut self, idx: usize) -> Option<&mut ChildWidget<T, Aug>> {
        let len1 = self.content1.len();
        if idx < len1 {
            self.content1.child_mut(idx)
        } else {
            self.content2.child_mut(idx - len1)
        }
    }

    fn last_child(&self) -> Option<&ChildWidget<T, Aug>> {
        self.content2
            .last_child()
            .or_else(|| self.content1.last_child())
    }

    fn len(&self) -> usize {
        self.content1.len() + self.content2.len()
    }
}

type ConditionFunc<T> = dyn Fn(&T, &Env)->bool;

pub struct ConditionalContent<T, Aug, ContentTrue, ContentFalse>{
    condition: Box<ConditionFunc<T>>,
    content_true: ContentTrue,
    content_false: ContentFalse,
    current: Option<bool>,
    phantom_aug: PhantomData<Aug>
}

impl <T: Data, Aug, ContentTrue> ConditionalContent<T, Aug, ContentTrue, StaticContent<T, Aug>>{
    pub fn new_if(cond: impl Fn(&T, &Env)->bool + 'static, content_true: ContentTrue) -> Self {
        ConditionalContent{
            condition: Box::new(cond),
            content_true,
            content_false: StaticContent::new(),
            current: None,
            phantom_aug: Default::default()
        }
    }
}

impl <T: Data, Aug, ContentTrue, ContentFalse> ConditionalContent<T, Aug, ContentTrue, ContentFalse>{
    pub fn new_if_else(cond: impl Fn(&T, &Env)->bool + 'static, content_true: ContentTrue, content_false: ContentFalse) -> Self {
        ConditionalContent{
            condition: Box::new(cond),
            content_true,
            content_false,
            current: None,
            phantom_aug: Default::default()
        }
    }
}


impl <T: Data, Aug, ContentTrue: Content<T, Aug>, ContentFalse: Content<T, Aug>> Content<T, Aug> for ConditionalContent<T, Aug, ContentTrue, ContentFalse> {
    fn content_added(&mut self, data: &T, env: &Env) {
        self.current = Some((*self.condition)(data, env));
    }

    fn update(&mut self, old_data: &T, data: &T, env: &Env) {
        if !old_data.same(data){
            self.content_added(data, env)
        }
    }

    fn add_child_widget(&mut self, _cw: ChildWidget<T, Aug>) -> bool {
        false
    }

    fn child_mut(&mut self, idx: usize) -> Option<&mut ChildWidget<T, Aug>> {
        if let Some(cond) = self.current{
            if cond {
                self.content_true.child_mut(idx)
            }else {
                self.content_false.child_mut(idx)
            }
        }else{
            None
        }
    }

    fn last_child(&self) -> Option<&ChildWidget<T, Aug>> {
        if let Some(cond) = self.current{
            if cond {
                self.content_true.last_child()
            }else {
                self.content_false.last_child()
            }
        }else{
            None
        }
    }

    fn len(&self) -> usize {
        if let Some(cond) = self.current{
            if cond {
                self.content_true.len()
            }else {
                self.content_false.len()
            }
        }else{
            0
        }
    }
}

pub struct ChildWidget<T, Aug> {
    pub(crate) widget: WidgetPod<T, Box<dyn Widget<T>>>,
    pub(crate) params: Aug,
}

impl<T: Data, Aug: Default + Clone + 'static> ChildWidget<T, Aug> {
    pub(crate) fn new(child: impl Widget<T> + 'static) -> Self {
        let params = child.augmentation().cloned().unwrap_or_else(Default::default);
        ChildWidget {
            widget: WidgetPod::new(Box::new(child)),
            params,
        }
    }

    pub(crate) fn new_with_params(child: impl Widget<T> + 'static, params: Aug) -> Self {
        ChildWidget {
            widget: WidgetPod::new(Box::new(child)),
            params,
        }
    }
}