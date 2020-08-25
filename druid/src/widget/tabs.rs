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
use std::slice::Iter;
use std::iter::FlatMap;

type TabsScope<TFD> = Scope<TabsScopePolicy<TFD>, Box<dyn Widget<TabsState<TFD>>>>;
type TabBodyPod<TFD> = WidgetPod<<TFD as TabsFromData>::T, <TFD as TabsFromData>::BodyWidget>;
type TabBarPod<TFD> = WidgetPod<TabsState<TFD>, Box<dyn Widget<TabsState<TFD>>>>;
type TabIndex = usize;

const MILLIS: u64 = 1_000_000; // Number of nanos

pub struct TabInfo{
    pub name: String,
    pub can_close: bool
}

impl TabInfo {
    pub fn new(name: String, can_close: bool) -> Self {
        TabInfo { name, can_close }
    }
}


/// A policy that determines how a Tabs instance derives its tabs from its app data
pub trait TabsFromData: Data {
    /// A type representing a set of tabs. Its expected to be cheap to derive and compare.
    /// Numeric types or small tuples of them are ideal.
    type TabSet: Eq;
    /// The identity of a tab.
    type TabKey: Hash + Eq + Clone + Debug;
    /// The data within this policy whilst it is being built.
    /// This is only be useful for implementations supporting AddTab, such as StaticTabs.
    /// It can be filled in with () by other implementations until associated type defaults are stable
    type Build;

    /// The input data that will a) be used to derive the tab and b) also be the input data of all the child widgets.
    type T : Data;

    /// The common type for all body widgets in this set of tabs.
    type BodyWidget: Widget<Self::T>;

    /// Derive the set of tabs from the data.
    fn tabs(&self, data: &Self::T) -> Self::TabSet;

    fn tabs_changed(&self, old_data: &Self::T, data: &Self::T) -> Option<Self::TabSet>{
        let cur = self.tabs(data);
        if cur != self.tabs(old_data){
            Some(cur)
        }else{
            None
        }
    }

    /// What are the current tabs set in order.
    fn keys_from_set(&self, set: Self::TabSet, data: &Self::T) -> Vec<Self::TabKey>;

    /// Presentation information for the tab
    fn info_from_key(&self, key: Self::TabKey, data: &Self::T) -> TabInfo;

    /// Body widget for the tab
    fn body_from_key(&self, key: Self::TabKey, data: &Self::T) -> Option<Self::BodyWidget>;

    #[allow(unused_variables)]
    fn close_tab(&self, key: Self::TabKey, data: &mut Self::T){

    }

    #[allow(unused_variables)]
    // This should only be implemented if supporting AddTab - possibly only StaticTabs needs to.
    fn build(build: Self::Build) -> Self {
        unimplemented!()
    }
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

impl <T: Data> Data for StaticTabs<T>{
    fn same(&self, _other: &Self) -> bool {
        // Changing the tabs after construction shouldn't be possible for static tabs
        // It seems pointless to compare them
        true
    }
}

impl<T: Data> TabsFromData for StaticTabs<T> {
    type TabSet = ();
    type TabKey = STabKey;
    type Build = Vec<InitialTab<T>>;
    type T = T;
    type BodyWidget = Box<dyn Widget<T>>;

    fn tabs(&self, _data: &T) -> Self::TabSet {
        ()
    }

    fn tabs_changed(&self, _old_data: &T, _data: &T) -> Option<Self::TabSet> {
        None
    }

    fn keys_from_set(&self, _set: Self::TabSet, _data: &T) -> Vec<Self::TabKey> {
        (0..self.tabs.len()).map(STabKey).collect()
    }

    fn info_from_key(&self, key: Self::TabKey, _data: &T) -> TabInfo {
        TabInfo::new(self.tabs[key.0].name.clone(), false)
    }

    fn body_from_key(&self, key: Self::TabKey, _data: &T) -> Option<Self::BodyWidget> {
        self.tabs[key.0].child.take()
    }

    fn build(build: Self::Build) -> Self {
        StaticTabs {
            tabs: Rc::new(build),
        }
    }
}

pub trait AddTab: TabsFromData {
    fn add_tab(tabs: &mut Self::Build, name: impl Into<String>, child: impl Widget<Self::T> + 'static);
}

impl<T: Data> AddTab for StaticTabs<T> {
    fn add_tab(tabs: &mut Self::Build, name: impl Into<String>, child: impl Widget<T> + 'static) {
        tabs.push(InitialTab::new(name, child))
    }
}

#[derive(Clone, Lens, Data)]
pub struct TabsState<TFD: TabsFromData> {
    pub inner: TFD::T,
    pub selected: TabIndex,
    pub tabs_from_data: TFD,
}

impl<TFD: TabsFromData> TabsState<TFD> {
    pub fn new(inner: TFD::T, selected: usize, tabs_from_data: TFD) -> Self {
        TabsState {
            inner,
            selected,
            tabs_from_data,
        }
    }
}

pub struct TabBar<TFD: TabsFromData> {
    axis: Axis,
    cross: CrossAxisAlignment,
    orientation: TabOrientation,
    tabs: Vec<(TFD::TabKey, TabBarPod<TFD>)>,
    hot: Option<TabIndex>,
    phantom_tfd: PhantomData<TFD>,
}

impl<TFD: TabsFromData> TabBar<TFD> {
    pub fn new(axis: Axis, cross: CrossAxisAlignment, orientation: TabOrientation) -> Self {
        TabBar {
            axis,
            cross,
            orientation,
            tabs: vec![],
            hot: None,
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

    fn ensure_tabs(&mut self, data: &TabsState<TFD>, tab_set: TFD::TabSet) {
        // Borrow checker fun
        let (orientation, axis, cross) = (self.orientation, self.axis, self.cross);
        let finish = |w| WidgetPod::new(orientation.rotate_and_box(w, axis, cross));
        let finish2 = |w| WidgetPod::new(orientation.rotate_and_box(w, axis, cross));

        ensure_for_tabs(
            &mut self.tabs,
            &data.tabs_from_data,
            tab_set,
            &data.inner,
            |tfd, key, _idx| {
                let info = tfd.info_from_key(key.clone(), &data.inner);

                let label = Label::<TabsState<TFD>>::new(&info.name[..])
                    .with_font("Gill Sans".to_string())
                    .with_text_color(Color::WHITE)
                    .with_text_size(12.0)
                    .padding(Insets::uniform_xy(9., 5.));

                if info.can_close{
                    let c_key = key.clone();
                    let row = Flex::row()
                        .with_child(label)
                        .with_child(Label::new( "ⓧ" ).on_click( move |_ctx, data : &mut TabsState<TFD>, _env|{
                            data.tabs_from_data.close_tab(c_key.clone(),  &mut data.inner);
                        }));
                    finish(row)
                }else{
                    finish2(label)
                }
            },
        );
    }
}

impl<TFD: TabsFromData> Widget<TabsState<TFD>> for TabBar<TFD> {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut TabsState<TFD>,
        env: &Env,
    ) {
        match event {
            Event::MouseDown(e) => {
                if let Some(idx) = self.find_idx(e.pos) {
                    data.selected = idx;
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
            _ => {}
        }

        for (_, tab) in self.tabs.iter_mut() {
            tab.event(ctx, event, data, env);
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &TabsState<TFD>,
        env: &Env,
    ) {
        if let LifeCycle::WidgetAdded = event {
            let init_set = data.tabs_from_data.tabs(&data.inner);
            self.ensure_tabs(data, init_set);
            ctx.children_changed();
            ctx.request_layout();
        }

        for  (_, tab) in self.tabs.iter_mut() {
            tab.lifecycle(ctx, event, data, env);
        }
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &TabsState<TFD>,
        data: &TabsState<TFD>,
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
        data: &TabsState<TFD>,
        env: &Env,
    ) -> Size {
        let (mut major, mut minor) = (0., 0.);
        for (_, tab) in self.tabs.iter_mut() {
            let size = tab.layout(ctx, bc, data, env);
            tab.set_layout_rect(
                ctx,
                data,
                env,
                Rect::from_origin_size(self.axis.pack(major, 0.), size),
            );
            major += self.axis.major(size);
            minor = f64::max(minor, self.axis.minor(size));
        }
        // Now go back through to reset the minors
        for (_, tab) in self.tabs.iter_mut() {
            let rect = tab.layout_rect();
            let rect = rect.with_size(self.axis.pack(self.axis.major(rect.size()), minor));
            tab.set_layout_rect(ctx, data, env, rect);
        }

        let wanted = self
            .axis
            .pack(f64::max(major, self.axis.major(bc.max())), minor);
        bc.constrain(wanted)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &TabsState<TFD>, env: &Env) {
        let hl_thickness = 2.;
        let highlight = env.get(theme::PRIMARY_LIGHT);
        // TODO: allow reversing tab order (makes more sense in some rotations)
        for (idx, (_, tab)) in self.tabs.iter_mut().enumerate() {
            let rect = tab.layout_rect();
            let rect = Rect::from_origin_size(rect.origin(), rect.size());
            let bg = match (idx == data.selected, Some(idx) == self.hot) {
                (_, true) => env.get(theme::BUTTON_DARK),
                (true, false) => env.get(theme::BACKGROUND_LIGHT),
                _ => env.get(theme::BACKGROUND_DARK),
            };
            ctx.fill(rect, &bg);

            tab.paint(ctx, data, env);
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

fn ensure_for_tabs<Content, TFD: TabsFromData + ?Sized>(
    contents: &mut Vec<(TFD::TabKey, Content)>,
    tfd: &TFD,
    tab_set: TFD::TabSet,
    data: &TFD::T,
    f: impl Fn(&TFD, TFD::TabKey, usize) -> Content,
) -> Vec<usize> {
    let mut existing_by_key: HashMap<TFD::TabKey, Content> = contents.drain(..).collect();

    let mut existing_idx = Vec::new();
    for (idx, key) in tfd.keys_from_set(tab_set, data).into_iter().enumerate() {
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

pub struct TabsBody<TFD: TabsFromData> {
    children: Vec<(TFD::TabKey, Option<TabBodyPod<TFD>>)>,
    transition: Option<TabsTransition>,
    axis: Axis,
    phantom_tfd: PhantomData<TFD>,
}

impl<TFD: TabsFromData> TabsBody<TFD> {
    pub fn new(axis: Axis) -> TabsBody<TFD> {
        TabsBody {
            children: vec![],
            transition: None,
            axis,
            phantom_tfd: Default::default(),
        }
    }

    fn make_tabs(&mut self, data: &TabsState<TFD>, tab_set: TFD::TabSet) -> Vec<usize> {
        ensure_for_tabs(
            &mut self.children,
            &data.tabs_from_data,
            tab_set,
            &data.inner,
            |tfd, key, idx| {
                tfd.body_from_key(key.clone(), &data.inner).map(WidgetPod::new)
                    // Make a dummy body
                    // Box::new(Label::new(format!(
                    //     "Could not create tab for key {:?} at index {}",
                    //     key, idx
                    // )))
                }
        )
    }

    fn active_child(&mut self, state: &TabsState<TFD>) -> Option<&mut TabBodyPod<TFD>> {
        Self::child(&mut self.children, state.selected)
    }

    // Doesn't take self to allow separate borrowing
    fn child(children: &mut Vec<(TFD::TabKey, Option<TabBodyPod<TFD>>)>, idx: usize) -> Option<&mut TabBodyPod<TFD>> {
        children.get_mut(idx).and_then(|x| x.1.as_mut() )
    }

    fn child_pods(&mut self) -> impl Iterator<Item=&mut TabBodyPod<TFD>> {
        self.children.iter_mut().flat_map(|x| x.1.as_mut())
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

impl<TFD: TabsFromData> Widget<TabsState<TFD>> for TabsBody<TFD> {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut TabsState<TFD>,
        env: &Env,
    ) {
        if hidden_should_receive_event(event) {
            for child in self.child_pods() {
                child.event(ctx, event, &mut data.inner, env);
            }
        } else if let Some(child) = self.active_child(data) {
            child.event(ctx, event, &mut data.inner, env);
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &TabsState<TFD>,
        env: &Env,
    ) {
        if let LifeCycle::WidgetAdded = event {
            let init_set = data.tabs_from_data.tabs(&data.inner);
            self.make_tabs(data, init_set);
            ctx.children_changed();
            ctx.request_layout();
        }

        if hidden_should_receive_lifecycle(event) {
            for child in self.child_pods() {
                child.lifecycle(ctx, event, &data.inner, env);
            }
        } else if let Some(child) = self.active_child(data) {
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
        old_data: &TabsState<TFD>,
        data: &TabsState<TFD>,
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
                if let Some(child) = Self::child(&mut self.children,idx) {
                    child.update(ctx, &data.inner, env)
                }
            }
        } else {
            for child in self.child_pods() {
                child.update(ctx, &data.inner, env);
            }
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &TabsState<TFD>,
        env: &Env,
    ) -> Size {
        if let Some(ref mut child) = self.active_child(data) {
            let inner = &data.inner;
            let size = child.layout(ctx, bc, inner, env);
            child.set_layout_rect(ctx, inner, env, Rect::from_origin_size(Point::ORIGIN, size));
            size
        }else{
            bc.max()
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &TabsState<TFD>, env: &Env) {
        if let Some(trans) = &self.transition {
            let axis = self.axis;
            let size = ctx.size();
            let major = axis.major(size);
            ctx.clip(Rect::from_origin_size(Point::ZERO, size));

            let children = &mut self.children;
            if let Some(ref mut prev) = Self::child(children, trans.previous_idx) {
                ctx.with_save(|ctx| {
                    ctx.transform(trans.previous_transform(axis, major));
                    prev.paint_raw(ctx, &data.inner, env);
                })
            }
            if let Some(ref mut child) = Self::child(children, data.selected) {
                ctx.with_save(|ctx| {
                    ctx.transform(trans.selected_transform(axis, major));
                    child.paint_raw(ctx, &data.inner, env);
                })
            }
        } else {
            if let Some(ref mut child) =  Self::child(&mut self.children,data.selected) {
                child.paint_raw(ctx, &data.inner, env);
            }
        }
    }
}

// This only needs to exist to be able to give a reasonable type to the TabScope
pub struct TabsScopePolicy<TFD> {
    tabs_from_data: TFD,
    selected: TabIndex,
}

impl<TFD> TabsScopePolicy<TFD> {
    pub fn new(tabs_from_data: TFD, selected: TabIndex) -> Self {
        Self {
            tabs_from_data,
            selected
        }
    }
}

impl<TFD: TabsFromData> ScopePolicy for TabsScopePolicy<TFD> {
    type In = TFD::T;
    type State = TabsState<TFD>;
    type Transfer = LensScopeTransfer<tabs_state_derived_lenses::inner, Self::In, Self::State>;

    fn create(self, inner: &Self::In) -> (Self::State, Self::Transfer) {
        (
            TabsState::new(inner.clone(), self.selected, self.tabs_from_data),
            LensScopeTransfer::new(Self::State::inner),
        )
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
    child: SingleUse<Box<dyn Widget<T>>>, // This is to avoid cloning provided tabs
}

impl<T: Data> InitialTab<T> {
    pub fn new(name: impl Into<String>, child: impl Widget<T> + 'static) -> Self {
        InitialTab {
            name: name.into(),
            child: SingleUse::new(Box::new(child)),
        }
    }
}

enum TabsContent<TFD: TabsFromData> {
    Building {
        tabs: TFD::Build,
    },
    Complete {
        tabs: TFD,
    },
    Running {
        scope: WidgetPod<TFD::T, TabsScope<TFD>>,
    },
    Swapping,
}

pub struct Tabs<TFD: TabsFromData> {
    axis: Axis,
    cross: CrossAxisAlignment, // Not sure if this should have another enum. Middle means nothing here
    rotation: TabOrientation,
    content: TabsContent<TFD>,
}

impl<T: Data> Tabs<StaticTabs<T>> {
    pub fn new() -> Self {
        Tabs::building(Vec::new())
    }
}

impl<TFD: TabsFromData> Tabs<TFD> {
    fn with_content(content: TabsContent<TFD>) -> Self {
        Tabs {
            axis: Axis::Horizontal,
            cross: CrossAxisAlignment::Start,
            rotation: TabOrientation::Standard,
            content,
        }
    }

    pub fn of(tabs: TFD) -> Self {
        Self::with_content(TabsContent::Complete { tabs })
    }

    pub fn building(tabs_from_data: TFD::Build) -> Self where TFD : AddTab {
        Self::with_content(TabsContent::Building {
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
        child: impl Widget<TFD::T> + 'static,
    ) -> Tabs<TFD>
    where
        TFD: AddTab,
    {
        self.add_tab(name, child);
        self
    }

    pub fn add_tab(&mut self, name: impl Into<String>, child: impl Widget<TFD::T> + 'static)
    where
        TFD: AddTab,
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

    pub fn with_tabs<TabsFromD: TabsFromData>(self, tabs: TabsFromD) -> Tabs<TabsFromD> {
        Tabs {
            axis: self.axis,
            cross: self.cross,
            rotation: self.rotation,
            content: TabsContent::Complete { tabs },
        }
    }

    pub fn make_scope(&self, tabs_from_data: TFD) -> WidgetPod<TFD::T, TabsScope<TFD>> {
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
        let mut layout: Flex<TabsState<TFD>> = Flex::for_axis(self.axis.cross());

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

impl<TFD: TabsFromData> Widget<TFD::T> for Tabs<TFD> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut TFD::T, env: &Env) {
        if let TabsContent::Running { scope } = &mut self.content {
            scope.event(ctx, event, data, env);
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &TFD::T, env: &Env) {
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

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &TFD::T, data: &TFD::T, env: &Env) {
        if let TabsContent::Running { scope } = &mut self.content {
            scope.update(ctx, data, env);
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &TFD::T, env: &Env) -> Size {
        if let TabsContent::Running { scope } = &mut self.content {
            let size = scope.layout(ctx, bc, data, env);
            scope.set_layout_rect(ctx, data, env, Rect::from_origin_size(Point::ORIGIN, size));
            size
        } else {
            bc.min()
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &TFD::T, env: &Env) {
        if let TabsContent::Running { scope } = &mut self.content {
            scope.paint(ctx, data, env)
        }
    }
}
