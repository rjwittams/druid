// Copyright 2020 The Druid Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A widget that can switch between one of many views, hiding the inactive ones.
//!

use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::kurbo::Line;
use crate::piet::RenderContext;

use crate::widget::{Axis, CrossAxisAlignment, Flex, Label, LensScopeTransfer, Scope, ScopePolicy};
use crate::{
    theme, Affine, BoxConstraints, Color, Data, Env, Event, EventCtx, Insets, LayoutCtx, Lens,
    LifeCycle, LifeCycleCtx, PaintCtx, Point, Rect, SingleUse, Size, UpdateCtx, Widget, WidgetExt,
    WidgetPod,
};

type TabsScope<T, TFD> = Scope<TabsScopePolicy<T, TFD>, Box<dyn Widget<TabsState<T, TFD>>>>;
pub type TabBodyPod<T> = WidgetPod<T, Box<dyn Widget<T>>>;
type TabBarPod = WidgetPod<TabIndex, Box<dyn Widget<TabIndex>>>;
type TabIndex = usize;

const MILLIS: u64 = 1_000_000; // Number of nanos

pub trait TabsFromData<T>: Clone + 'static {
    type TabSet;
    type TabKey: Hash + Eq + Clone + Debug;
    type Build; // This may only be useful for static tabs.
                // It can be filled in with () until associated type defaults are stable

    fn build(_build: Self::Build) -> Self {
        unimplemented!()
    }
    fn initial_tabs(&self, data: &T) -> Self::TabSet;
    fn tabs_changed(&self, old_data: &T, data: &T) -> Option<Self::TabSet>;
    fn keys_from_set(&self, set: Self::TabSet) -> Vec<Self::TabKey>;
    fn name_from_key(&self, key: Self::TabKey) -> String;
    fn body_from_key(&self, key: Self::TabKey) -> Option<TabBodyPod<T>>;
}

#[derive(Clone)]
pub struct StaticTabs<T> {
    // This needs be able to avoid cloning the widgets we are given -
    // as such it is Rc
    tabs: Rc<Vec<InitialTab<T>>>,
}

impl<T> StaticTabs<T> {
    pub fn new() -> Self {
        StaticTabs {
            tabs: Rc::new(Vec::new()),
        }
    }
}

#[derive(Data, Copy, Clone, Debug, PartialOrd, PartialEq, Eq, Hash)]
pub struct STabKey(pub usize);

impl<T: Data> TabsFromData<T> for StaticTabs<T> {
    type TabSet = ();
    type TabKey = STabKey;
    type Build = Vec<InitialTab<T>>;

    fn build(build: Self::Build) -> Self {
        StaticTabs {
            tabs: Rc::new(build),
        }
    }

    fn initial_tabs(&self, _data: &T) -> Self::TabSet {
        ()
    }

    fn tabs_changed(&self, _old_data: &T, _data: &T) -> Option<Self::TabSet> {
        None
    }

    fn keys_from_set(&self, _set: Self::TabSet) -> Vec<Self::TabKey> {
        (0..self.tabs.len()).map(STabKey).collect()
    }

    fn name_from_key(&self, key: Self::TabKey) -> String {
        self.tabs[key.0].name.clone()
    }

    fn body_from_key(&self, key: Self::TabKey) -> Option<TabBodyPod<T>> {
        self.tabs[key.0].child.take()
    }
}

pub trait AddTab<T>: TabsFromData<T> {
    fn add_tab(tabs: &mut Self::Build, name: impl Into<String>, child: impl Widget<T> + 'static);
}

impl<T: Data> AddTab<T> for StaticTabs<T> {
    fn add_tab(tabs: &mut Self::Build, name: impl Into<String>, child: impl Widget<T> + 'static) {
        tabs.push(InitialTab::new(name, child))
    }
}

#[derive(Clone, Lens)]
pub struct TabsState<T: Data, TFD: TabsFromData<T>> {
    pub inner: T,
    pub selected: TabIndex,
    pub tabs_from_data: TFD,
}

impl<T: Data, TFD: TabsFromData<T>> Data for TabsState<T, TFD> {
    fn same(&self, other: &Self) -> bool {
        return self.inner.same(&other.inner) && self.selected.same(&other.selected);
    }
}

impl<T: Data, TFD: TabsFromData<T>> TabsState<T, TFD> {
    pub fn new(inner: T, selected: usize, tabs_from_data: TFD) -> Self {
        TabsState {
            inner,
            selected,
            tabs_from_data,
        }
    }
}

pub struct TabBar<T, TFD: TabsFromData<T>> {
    axis: Axis,
    cross: CrossAxisAlignment,
    orientation: TabOrientation,
    tabs: Vec<(TFD::TabKey, TabBarPod)>,
    hot: Option<TabIndex>,
    phantom_t: PhantomData<T>,
    phantom_tfd: PhantomData<TFD>,
}

impl<T: Data, TFD: TabsFromData<T>> TabBar<T, TFD> {
    pub fn new(axis: Axis, cross: CrossAxisAlignment, orientation: TabOrientation) -> Self {
        TabBar {
            axis,
            cross,
            orientation,
            tabs: vec![],
            hot: None,
            phantom_t: Default::default(),
            phantom_tfd: Default::default(),
        }
    }

    pub fn find_idx(&self, pos: Point) -> Option<TabIndex> {
        let major_pix = self.axis.major_pos(pos);
        let axis = self.axis;
        let res = self
            .tabs
            .binary_search_by_key(&((major_pix * 10.) as i64), |(_, tab)| {
                let rect = tab.layout_rect();
                let far_pix = axis.major_pos(rect.origin()) + axis.major(rect.size());
                (far_pix * 10.) as i64
            });
        match res {
            Ok(idx) => Some(idx),
            Err(idx) if idx < self.tabs.len() => Some(idx),
            _ => None,
        }
    }

    fn ensure_tabs(&mut self, data: &TabsState<T, TFD>, tab_set: TFD::TabSet) {
        // Borrow checker fun
        let (orientation, axis, cross) = (self.orientation, self.axis, self.cross);
        let rotate = |w| orientation.rotate_and_box(w, axis, cross);

        ensure_for_tabs(
            &mut self.tabs,
            &data.tabs_from_data,
            tab_set,
            |tfd, key, _| {
                let name = tfd.name_from_key(key);
                let label = Label::<usize>::new(&name[..])
                    .with_font("Gill Sans".to_string())
                    .with_text_color(Color::WHITE)
                    .with_text_size(12.0)
                    .padding(Insets::uniform_xy(9., 5.));
                WidgetPod::new(rotate(label))
            },
        );
    }
}

impl<T: Data, TFD: TabsFromData<T>> Widget<TabsState<T, TFD>> for TabBar<T, TFD> {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut TabsState<T, TFD>,
        env: &Env,
    ) {
        match event {
            Event::MouseDown(e) => {
                if let Some(idx) = self.find_idx(e.pos) {
                    data.selected = idx;
                    ctx.is_handled();
                }
            }
            Event::MouseMove(e) => {
                let new_hot = if ctx.is_hot() {
                    self.find_idx(e.pos)
                } else {
                    None
                };
                if new_hot != self.hot {
                    self.hot = new_hot;
                    ctx.request_paint();
                }
            }
            _ => {
                for (mut idx, (_, tab)) in self.tabs.iter_mut().enumerate() {
                    tab.event(ctx, event, &mut idx, env);
                }
            }
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &TabsState<T, TFD>,
        env: &Env,
    ) {
        if let LifeCycle::WidgetAdded = event {
            let init_set = data.tabs_from_data.initial_tabs(&data.inner);
            self.ensure_tabs(data, init_set);
            ctx.children_changed();
            ctx.request_layout();
        }

        for (mut idx, (_, tab)) in self.tabs.iter_mut().enumerate() {
            tab.lifecycle(ctx, event, &mut idx, env);
        }
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &TabsState<T, TFD>,
        data: &TabsState<T, TFD>,
        _env: &Env,
    ) {
        let changed_tabs = data
            .tabs_from_data
            .tabs_changed(&old_data.inner, &data.inner);
        if let Some(tab_set) = changed_tabs {
            self.ensure_tabs(data, tab_set);
            ctx.children_changed();
            ctx.request_layout();
        } else if old_data.selected != data.selected {
            ctx.request_paint();
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &TabsState<T, TFD>,
        env: &Env,
    ) -> Size {
        let (mut major, mut minor) = (0., 0.);
        for (idx, (_, tab)) in self.tabs.iter_mut().enumerate() {
            let size = tab.layout(ctx, bc, &idx, env);
            tab.set_layout_rect(
                ctx,
                &idx,
                env,
                Rect::from_origin_size(self.axis.pack(major, 0.), size),
            );
            major += self.axis.major(size);
            minor = f64::max(minor, self.axis.minor(size));
        }
        // Now go back through to reset the minors
        for (idx, (_, tab)) in self.tabs.iter_mut().enumerate() {
            let rect = tab.layout_rect();
            let rect = rect.with_size(self.axis.pack(self.axis.major(rect.size()), minor));
            tab.set_layout_rect(ctx, &idx, env, rect);
        }

        let wanted = self
            .axis
            .pack(f64::max(major, self.axis.major(bc.max())), minor);
        bc.constrain(wanted)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &TabsState<T, TFD>, env: &Env) {
        let hl_thickness = 2.;
        let highlight = env.get(theme::PRIMARY_LIGHT);
        for (idx, (_, tab)) in self.tabs.iter_mut().enumerate() {
            let rect = tab.layout_rect();
            let rect = Rect::from_origin_size(rect.origin(), rect.size());
            let bg = match (idx == data.selected, Some(idx) == self.hot) {
                (_, true) => env.get(theme::BUTTON_DARK),
                (true, false) => env.get(theme::BACKGROUND_LIGHT),
                _ => env.get(theme::BACKGROUND_DARK),
            };
            ctx.fill(rect, &bg);

            tab.paint(ctx, &idx, env);
            if idx == data.selected {
                let (maj_near, maj_far) = self.axis.major_span(&rect);
                let (min_near, min_far) = self.axis.minor_span(&rect);
                let minor_pos = if let CrossAxisAlignment::End = self.cross {
                    min_near + (hl_thickness / 2.)
                } else {
                    min_far - (hl_thickness / 2.)
                };

                ctx.stroke(
                    Line::new(
                        self.axis.pack(maj_near, minor_pos),
                        self.axis.pack(maj_far, minor_pos),
                    ),
                    &highlight,
                    hl_thickness,
                )
            }
        }
    }
}

pub struct TabsTransition {
    previous_idx: TabIndex,
    current_time: u64,
    length: u64,
    increasing: bool,
}

impl TabsTransition {
    pub fn new(previous_idx: TabIndex, length: u64, increasing: bool) -> Self {
        TabsTransition {
            previous_idx,
            current_time: 0,
            length,
            increasing,
        }
    }

    pub fn live(&self) -> bool {
        self.current_time < self.length
    }

    pub fn fraction(&self) -> f64 {
        (self.current_time as f64) / (self.length as f64)
    }

    pub fn previous_transform(&self, axis: Axis, main: f64) -> Affine {
        let x = if self.increasing {
            -main * self.fraction()
        } else {
            main * self.fraction()
        };
        Affine::translate(axis.pack(x, 0.))
    }

    pub fn selected_transform(&self, axis: Axis, main: f64) -> Affine {
        let x = if self.increasing {
            main * (1.0 - self.fraction())
        } else {
            -main * (1.0 - self.fraction())
        };
        Affine::translate(axis.pack(x, 0.))
    }
}

fn ensure_for_tabs<Content, T, TFD: TabsFromData<T> + ?Sized>(
    contents: &mut Vec<(TFD::TabKey, Content)>,
    tfd: &TFD,
    tab_set: TFD::TabSet,
    f: impl Fn(&TFD, TFD::TabKey, usize) -> Content,
) -> Vec<usize> {
    let mut existing_by_key: HashMap<TFD::TabKey, Content> = contents.drain(..).collect();

    let mut existing_idx = Vec::new();
    for (idx, key) in tfd.keys_from_set(tab_set).into_iter().enumerate() {
        let next = if let Some(child) = existing_by_key.remove(&key) {
            existing_idx.push(contents.len());
            child
        } else {
            f(&tfd, key.clone(), idx)
        };
        contents.push((key.clone(), next))
    }
    existing_idx
}

pub struct TabsBody<T, TFD: TabsFromData<T>> {
    children: Vec<(TFD::TabKey, TabBodyPod<T>)>,
    transition: Option<TabsTransition>,
    axis: Axis,
    phantom_tfd: PhantomData<TFD>,
}

impl<T: Data, TFD: TabsFromData<T>> TabsBody<T, TFD> {
    pub fn new(axis: Axis) -> TabsBody<T, TFD> {
        TabsBody {
            children: vec![],
            transition: None,
            axis,
            phantom_tfd: Default::default(),
        }
    }

    fn make_tabs(&mut self, data: &TabsState<T, TFD>, tab_set: TFD::TabSet) -> Vec<usize> {
        ensure_for_tabs(
            &mut self.children,
            &data.tabs_from_data,
            tab_set,
            |tfd, key, idx| {
                if let Some(body) = tfd.body_from_key(key.clone()) {
                    body
                } else {
                    // Make a dummy body
                    WidgetPod::<T, Box<dyn Widget<T>>>::new(Box::new(Label::new(format!(
                        "Could not create tab for key {:?} at index {}",
                        key, idx
                    ))))
                }
            },
        )
    }
}

impl<T: Data, TFD: TabsFromData<T>> TabsBody<T, TFD> {
    fn active(&mut self, state: &TabsState<T, TFD>) -> Option<&mut (TFD::TabKey, TabBodyPod<T>)> {
        self.children.get_mut(state.selected)
    }
}

fn hidden_should_receive_event(evt: &Event) -> bool {
    match evt {
        Event::WindowConnected
        | Event::WindowSize(_)
        | Event::Timer(_)
        | Event::Command(_)
        | Event::Internal(_) => true,
        Event::MouseDown(_)
        | Event::MouseUp(_)
        | Event::MouseMove(_)
        | Event::Wheel(_)
        | Event::KeyDown(_)
        | Event::KeyUp(_)
        | Event::Paste(_)
        | Event::Zoom(_) => false,
    }
}

fn hidden_should_receive_lifecycle(lc: &LifeCycle) -> bool {
    match lc {
        LifeCycle::WidgetAdded | LifeCycle::Internal(_) => true,
        LifeCycle::Size(_)
        | LifeCycle::AnimFrame(_)
        | LifeCycle::HotChanged(_)
        | LifeCycle::FocusChanged(_) => false,
    }
}

impl<T: Data, TFD: TabsFromData<T>> Widget<TabsState<T, TFD>> for TabsBody<T, TFD> {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut TabsState<T, TFD>,
        env: &Env,
    ) {
        if hidden_should_receive_event(event) {
            for (_, child) in &mut self.children {
                child.event(ctx, event, &mut data.inner, env);
            }
        } else if let Some((_, child)) = self.active(data) {
            child.event(ctx, event, &mut data.inner, env);
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &TabsState<T, TFD>,
        env: &Env,
    ) {
        if let LifeCycle::WidgetAdded = event {
            let init_set = data.tabs_from_data.initial_tabs(&data.inner);
            self.make_tabs(data, init_set);
            ctx.children_changed();
            ctx.request_layout();
        }

        if hidden_should_receive_lifecycle(event) {
            for (_, child) in &mut self.children {
                child.lifecycle(ctx, event, &data.inner, env);
            }
        } else if let Some((_, child)) = self.active(data) {
            // Pick which events go to all and which just to active
            child.lifecycle(ctx, event, &data.inner, env);
        }

        if let (Some(trans), LifeCycle::AnimFrame(interval)) = (&mut self.transition, event) {
            trans.current_time += *interval;
            if trans.live() {
                ctx.request_anim_frame();
            } else {
                self.transition = None;
            }
        }
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &TabsState<T, TFD>,
        data: &TabsState<T, TFD>,
        env: &Env,
    ) {
        let changed_tabs = data
            .tabs_from_data
            .tabs_changed(&old_data.inner, &data.inner);
        let init = if let Some(tab_set) = changed_tabs {
            ctx.children_changed();
            ctx.request_layout();
            Some(self.make_tabs(data, tab_set))
        } else {
            None
        };

        if old_data.selected != data.selected {
            self.transition = Some(TabsTransition::new(
                old_data.selected,
                250 * MILLIS,
                old_data.selected < data.selected,
            ));
            ctx.request_layout();
            ctx.request_anim_frame();
        }

        // Make sure to only pass events to initialised children
        if let Some(init) = init {
            for idx in init {
                if let Some((_, child)) = self.children.get_mut(idx) {
                    child.update(ctx, &data.inner, env)
                }
            }
        } else {
            for (_, child) in &mut self.children {
                child.update(ctx, &data.inner, env);
            }
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &TabsState<T, TFD>,
        env: &Env,
    ) -> Size {
        match self.active(data) {
            Some((_, ref mut child)) => {
                let inner = &data.inner;
                let size = child.layout(ctx, bc, inner, env);
                child.set_layout_rect(ctx, inner, env, Rect::from_origin_size(Point::ORIGIN, size));
                size
            }
            None => bc.max(),
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &TabsState<T, TFD>, env: &Env) {
        if let Some(trans) = &self.transition {
            let axis = self.axis;
            let size = ctx.size();
            let major = axis.major(size);
            ctx.clip(Rect::from_origin_size(Point::ZERO, size));

            let children = &mut self.children;
            if let Some((_, ref mut prev)) = children.get_mut(trans.previous_idx) {
                ctx.with_save(|ctx| {
                    ctx.transform(trans.previous_transform(axis, major));
                    prev.paint_raw(ctx, &data.inner, env);
                })
            }
            if let Some((_, ref mut child)) = children.get_mut(data.selected) {
                ctx.with_save(|ctx| {
                    ctx.transform(trans.selected_transform(axis, major));
                    child.paint_raw(ctx, &data.inner, env);
                })
            }
        } else {
            if let Some((_, ref mut child)) = self.children.get_mut(data.selected) {
                child.paint_raw(ctx, &data.inner, env);
            }
        }
    }
}

pub struct TabsScopePolicy<T, TFD> {
    tabs_from_data: TFD,
    selected: TabIndex,
    phantom_t: PhantomData<T>,
}

impl<T, TFD: 'static> TabsScopePolicy<T, TFD> {
    pub fn new(tabs_from_data: TFD, selected: TabIndex) -> Self {
        Self {
            tabs_from_data,
            selected,
            phantom_t: Default::default(),
        }
    }
}

impl<T: Data, TFD: TabsFromData<T>> ScopePolicy for TabsScopePolicy<T, TFD> {
    type In = T;
    type State = TabsState<T, TFD>;
    type Transfer = LensScopeTransfer<tabs_state_derived_lenses::inner, T, Self::State>;

    fn create(self, inner: &Self::In) -> (Self::State, Self::Transfer) {
        let state = TabsState::new(inner.clone(), self.selected, self.tabs_from_data);
        (state, LensScopeTransfer::new(Self::State::inner))
    }
}

#[derive(Data, Copy, Clone, Debug, PartialOrd, PartialEq)]
pub enum TabOrientation {
    Standard,
    Turns(u8), // These represent 90 degree rotations clockwise.
}

impl TabOrientation {
    pub fn rotate_and_box<W: Widget<T> + 'static, T: Data>(
        self,
        widget: W,
        axis: Axis,
        cross: CrossAxisAlignment,
    ) -> Box<dyn Widget<T>> {
        let turns = match self {
            Self::Standard => match (axis, cross) {
                (Axis::Horizontal, _) => 0,
                (Axis::Vertical, CrossAxisAlignment::Start) => 3,
                (Axis::Vertical, _) => 1,
            },
            Self::Turns(turns) => turns,
        };

        if turns == 0 {
            Box::new(widget)
        } else {
            Box::new(widget.rotate(turns))
        }
    }
}

pub struct InitialTab<T> {
    name: String,
    child: SingleUse<TabBodyPod<T>>, // This is to avoid cloning provided tabs
}

impl<T: 'static> InitialTab<T> {
    pub fn new(name: impl Into<String>, child: impl Widget<T> + 'static) -> Self {
        InitialTab {
            name: name.into(),
            child: SingleUse::new(WidgetPod::new(Box::new(child))),
        }
    }
}

pub enum TabsContent<T: Data, TFD: TabsFromData<T>> {
    Building {
        tabs: TFD::Build,
    },
    Complete {
        tabs: TFD,
    },
    Running {
        scope: WidgetPod<T, TabsScope<T, TFD>>,
    },
    Swapping,
}

pub struct Tabs<T: Data, TFD: TabsFromData<T> = StaticTabs<T>> {
    axis: Axis,
    cross: CrossAxisAlignment, // Not sure if this should have another enum. Middle means nothing here
    rotation: TabOrientation,
    content: TabsContent<T, TFD>,
}

impl<T: Data> Tabs<T, StaticTabs<T>> {
    pub fn new() -> Self {
        Tabs::building(Vec::new())
    }
}

impl<T: Data, TFD: TabsFromData<T>> Tabs<T, TFD> {
    pub fn with_content(content: TabsContent<T, TFD>) -> Self {
        Tabs {
            axis: Axis::Horizontal,
            cross: CrossAxisAlignment::Start,
            rotation: TabOrientation::Standard,
            content
        }
    }

    pub fn of(tabs: TFD)->Self{
        Self::with_content( TabsContent::Complete {tabs} )
    }

    pub fn building(tabs_from_data: TFD::Build) -> Self {
        Self::with_content( TabsContent::Building {
                tabs: tabs_from_data,
            })
    }

    pub fn with_axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    pub fn with_rotation(mut self, rotation: TabOrientation) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn with_cross_axis_alignment(mut self, cross: CrossAxisAlignment) -> Self {
        self.cross = cross;
        self
    }

    pub fn with_tab(
        mut self,
        name: impl Into<String>,
        child: impl Widget<T> + 'static,
    ) -> Tabs<T, TFD>
    where
        TFD: AddTab<T>,
    {
        self.add_tab(name, child);
        self
    }

    pub fn add_tab(&mut self, name: impl Into<String>, child: impl Widget<T> + 'static)
    where
        TFD: AddTab<T>,
    {
        if let TabsContent::Building {
            tabs: tabs_from_data,
        } = &mut self.content
        {
            TFD::add_tab(tabs_from_data, name, child)
        } else {
            log::warn!("Can't add static tabs to a running or complete tabs instance!")
        }
    }

    pub fn with_tabs<TabsFromD: TabsFromData<T>>(self, tabs: TabsFromD) -> Tabs<T, TabsFromD> {
        Tabs {
            axis: self.axis,
            cross: self.cross,
            rotation: self.rotation,
            content: TabsContent::Complete { tabs },
        }
    }

    pub fn make_scope(&self, tabs_from_data: TFD) -> WidgetPod<T, TabsScope<T, TFD>> {
        let (bar, body) = (
            (TabBar::new(self.axis, self.cross, self.rotation), 0.0),
            (
                TabsBody::new(self.axis)
                    .padding(5.)
                    .border(theme::BORDER_DARK, 0.5)
                    .expand(),
                1.0,
            ),
        );
        let mut layout: Flex<TabsState<T, TFD>> = Flex::for_axis(self.axis.cross());

        if let CrossAxisAlignment::End = self.cross {
            layout.add_flex_child(body.0, body.1);
            layout.add_flex_child(bar.0, bar.1);
        } else {
            layout.add_flex_child(bar.0, bar.1);
            layout.add_flex_child(body.0, body.1);
        };

        WidgetPod::new(Scope::new(
            TabsScopePolicy::new(tabs_from_data, 0),
            Box::new(layout),
        ))
    }
}

impl<T: Data, TFD: TabsFromData<T>> Widget<T> for Tabs<T, TFD> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        if let TabsContent::Running { scope } = &mut self.content {
            scope.event(ctx, event, data, env);
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        if let LifeCycle::WidgetAdded = event {
            let mut temp = TabsContent::Swapping;
            std::mem::swap(&mut self.content, &mut temp);

            self.content = match temp {
                TabsContent::Building { tabs } => {
                    ctx.children_changed();
                    TabsContent::Running {
                        scope: self.make_scope(TFD::build(tabs)),
                    }
                }
                TabsContent::Complete { tabs } => {
                    ctx.children_changed();
                    TabsContent::Running {
                        scope: self.make_scope(tabs),
                    }
                }
                _ => temp,
            };
        }
        if let TabsContent::Running { scope } = &mut self.content {
            scope.lifecycle(ctx, event, data, env)
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        if let TabsContent::Running { scope } = &mut self.content {
            scope.update(ctx, data, env);
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        if let TabsContent::Running { scope } = &mut self.content {
            let size = scope.layout(ctx, bc, data, env);
            scope.set_layout_rect(ctx, data, env, Rect::from_origin_size(Point::ORIGIN, size));
            size
        } else {
            bc.min()
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        if let TabsContent::Running { scope } = &mut self.content {
            scope.paint(ctx, data, env)
        }
    }
}
