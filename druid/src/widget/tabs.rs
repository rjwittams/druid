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

use crate::piet::RenderContext;
use crate::widget::{Axis, CrossAxisAlignment, Flex, Label, Scope, ScopePolicy};
use crate::{theme, Insets, Affine};
use crate::{
    BoxConstraints, Color, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx,
    PaintCtx, Point, Rect, Size, UpdateCtx, Widget, WidgetExt, WidgetPod,
};
use std::marker::PhantomData;

type TabsScope<T> = Scope<TabsScopePolicy<T>, Box<dyn Widget<TabsState<T>>>>;
type TabBodyPod<T> = WidgetPod<T, Box<dyn Widget<T>>>;
type TabBarPod = WidgetPod<TabIndex, Box<dyn Widget<TabIndex>>>;
type TabIndex = usize;
use crate::kurbo::Line;
use druid::im::Vector;
use TabsContent::*;

const MILLIS: u64 = 1_000_000; // Number of nanos

#[derive(Data, Clone)]
pub struct TabsState<T: Data> {
    pub inner: T,
    pub selected: TabIndex,
    pub names: Vector<String>, // Not sure if this should be here or internal to tab bar
}

impl<T: Data> TabsState<T> {
    pub fn new(inner: T, selected: usize, names: Vector<String>) -> Self {
        TabsState {
            inner,
            selected,
            names,
        }
    }
}

pub struct TabBar<T> {
    axis: Axis,
    cross: CrossAxisAlignment,
    orientation: TabOrientation,
    tabs: Vec<TabBarPod>,
    hot: Option<TabIndex>,
    phantom_t: PhantomData<T>,
}

impl<T: Data> TabBar<T> {
    pub fn new(axis: Axis, cross: CrossAxisAlignment, orientation: TabOrientation) -> Self {
        TabBar {
            axis,
            cross,
            orientation,
            tabs: vec![],
            hot: None,
            phantom_t: Default::default(),
        }
    }

    pub fn find_idx(&self, pos: Point) -> Option<TabIndex> {
        let major_pix = self.axis.major_pos(pos);
        let axis = self.axis;
        let res = self.tabs
            .binary_search_by_key(&((major_pix * 10.) as i64), |tab| {
                let rect = tab.layout_rect();
                let far_pix = axis.major_pos(rect.origin()) + axis.major(rect.size());
                (far_pix * 10.) as i64
            });
        match res {
            Ok(idx) => Some(idx),
            Err(idx) if idx < self.tabs.len()  => {
                Some(idx)
            },
            _ => None,
        }
    }

    fn make_tabs(&mut self, data: &TabsState<T>) {
        self.tabs.clear();
        for (_idx, name) in data.names.iter().enumerate() {
            let label = Label::<usize>::new(&name[..])
                .with_font("Gill Sans".to_string())
                .with_text_color(Color::WHITE)
                .with_text_size(12.0)
                .padding(Insets::uniform_xy(9., 5.));
            let rot = self.orientation.rotate_and_box(label, self.axis, self.cross);

            self.tabs.push(WidgetPod::new(rot));
        }
    }
}

impl<T: Data> Widget<TabsState<T>> for TabBar<T> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut TabsState<T>, env: &Env) {
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
                for (mut idx, tab) in self.tabs.iter_mut().enumerate() {
                    tab.event(ctx, event, &mut idx, env);
                }
            }
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &TabsState<T>,
        env: &Env,
    ) {
        if let LifeCycle::WidgetAdded = event {
            self.make_tabs(data);
            ctx.children_changed();
            ctx.request_layout();
        }

        for (mut idx, tab) in self.tabs.iter_mut().enumerate() {
            tab.lifecycle(ctx, event, &mut idx, env);
        }
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &TabsState<T>,
        data: &TabsState<T>,
        _env: &Env,
    ) {
        if old_data.selected != data.selected {
            ctx.request_paint();
        }
        if old_data.names != data.names {
            self.make_tabs(data);
            ctx.children_changed();
            ctx.request_layout();
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &TabsState<T>,
        env: &Env,
    ) -> Size {
        let (mut major, mut minor) = (0., 0.);
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
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
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            let rect = tab.layout_rect();
            let rect = rect.with_size(self.axis.pack(self.axis.major(rect.size()), minor));
            tab.set_layout_rect(ctx, &idx, env, rect);
        }

        let wanted = self
            .axis
            .pack(f64::max(major, self.axis.major(bc.max())), minor);
        bc.constrain(wanted)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &TabsState<T>, env: &Env) {
        let hl_thickness = 2.;
        let highlight = env.get(theme::PRIMARY_LIGHT);
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            let rect = tab.layout_rect();
            let rect = Rect::from_origin_size( rect.origin(), rect.size() );
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

pub struct TabsTransition{
    previous_idx: TabIndex,
    current_time: u64,
    length: u64,
    increasing: bool
}

impl TabsTransition {
    pub fn new(previous_idx: TabIndex, length: u64, increasing: bool) -> Self {
        TabsTransition {previous_idx, current_time: 0, length, increasing: increasing }
    }

    pub fn live(&self)->bool{
        self.current_time < self.length
    }

    pub fn fraction(&self)->f64{
        (self.current_time as f64) / (self.length as f64)
    }

    pub fn previous_transform(&self, axis: Axis, main: f64) ->Affine{
        let x = if self.increasing {
            - main * self.fraction()
        }else{
            main * self.fraction()
        };
        Affine::translate(axis.pack(x, 0.))
    }

    pub fn selected_transform(&self, axis: Axis,  main: f64) ->Affine{
        let x = if self.increasing {
            main * (1.0 - self.fraction())
        }else {
            -main * (1.0 - self.fraction())
        };
        Affine::translate(axis.pack(x, 0.))
    }
}

pub struct TabsBody<T> {
    children: Vec<TabBodyPod<T>>,
    transition: Option<TabsTransition>,
    axis: Axis
}

impl<T> TabsBody<T> {
    pub fn new(axis : Axis) -> TabsBody<T> {
        TabsBody { children: vec![], transition: None, axis }
    }

    pub fn with_child(mut self, child: impl Widget<T> + 'static) -> TabsBody<T> {
        self.add_child(child);
        self
    }

    pub fn add_child(&mut self, child: impl Widget<T> + 'static) {
        self.add_pod(WidgetPod::new(Box::new(child)));
    }

    pub fn add_pod(&mut self, pod: TabBodyPod<T>) {
        self.children.push(pod)
    }
}

impl<T: Data> TabsBody<T> {
    fn active(&mut self, state: &TabsState<T>) -> Option<&mut TabBodyPod<T>> {
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


impl<T: Data> Widget<TabsState<T>> for TabsBody<T> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut TabsState<T>, env: &Env) {
        if hidden_should_receive_event(event) {
            for child in &mut self.children {
                child.event(ctx, event, &mut data.inner, env);
            }
        } else if let Some(child) = self.active(data) {
            child.event(ctx, event, &mut data.inner, env);
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &TabsState<T>,
        env: &Env,
    ) {
        if hidden_should_receive_lifecycle(event) {
            for child in &mut self.children {
                child.lifecycle(ctx, event, &data.inner, env);
            }
        } else if let Some(child) = self.active(data) {
            // Pick which events go to all and which just to active
            child.lifecycle(ctx, event, &data.inner, env);
        }

        if let (Some(trans), LifeCycle::AnimFrame(interval)) = (&mut self.transition, event) {
            trans.current_time += *interval;
            if trans.live() {
                ctx.request_anim_frame();

            }else{
                self.transition = None;
                log::info!("Finished at {:?}", std::time::Instant::now() );
            }
        }
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        _old_data: &TabsState<T>,
        data: &TabsState<T>,
        env: &Env,
    ) {
        if _old_data.selected != data.selected {
            self.transition = Some(TabsTransition::new(_old_data.selected, 250 * MILLIS, _old_data.selected < data.selected));
            log::info!("Started at {:?}", std::time::Instant::now() );
            ctx.request_layout();
            ctx.request_anim_frame();
        }
        for child in &mut self.children {
            child.update(ctx, &data.inner, env);
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &TabsState<T>,
        env: &Env,
    ) -> Size {
        match self.active(data) {
            Some(ref mut child) => {
                let inner = &data.inner;
                let size = child.layout(ctx, bc, inner, env);
                child.set_layout_rect(ctx, inner, env, Rect::from_origin_size(Point::ORIGIN, size));
                size
            }
            None => bc.max(),
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &TabsState<T>, env: &Env) {

        if let Some(trans) = &self.transition{
            let axis = self.axis;
            let size = ctx.size();
            let major = axis.major(size);
            ctx.clip(Rect::from_origin_size (Point::ZERO, size));

            let children = &mut self.children;
            if let Some(ref mut prev) = children.get_mut(trans.previous_idx) {
                ctx.with_save(|ctx| {

                    ctx.transform( trans.previous_transform(axis, major) );
                    prev.paint_raw(ctx, &data.inner, env);
                })
            }
            if let Some(ref mut child) = children.get_mut(data.selected) {
                ctx.with_save(|ctx| {
                    ctx.transform(trans.selected_transform(axis, major));
                    child.paint_raw(ctx, &data.inner, env);
                })
            }
        }else {
            if let Some(ref mut child) = self.children.get_mut(data.selected) {
                child.paint_raw(ctx, &data.inner, env);
            }
        }
    }
}

pub struct TabsScopePolicy<T> {
    names: Vector<String>,
    selected: TabIndex,
    phantom_t: PhantomData<T>,
}

impl<T> TabsScopePolicy<T> {
    pub fn new(names: Vector<String>, selected: TabIndex) -> Self {
        TabsScopePolicy {
            names,
            selected,
            phantom_t: Default::default(),
        }
    }
}

// Would be easy to generate with a proc macro
impl<T: Data> ScopePolicy for TabsScopePolicy<T> {
    type In = T;
    type State = TabsState<T>;

    fn default_state(&self, inner: &Self::In) -> Self::State {
        TabsState::new(inner.clone(), self.selected, self.names.clone())
    }

    fn replace_in_state(&self, state: &mut Self::State, inner: &Self::In) {
        state.inner = inner.clone();
    }

    fn write_back_input(&self, state: &Self::State, inner: &mut Self::In) {
        *inner = state.inner.clone();
    }
}

#[derive(Data, Copy, Clone, Debug, PartialOrd, PartialEq)]
pub enum TabOrientation {
    Standard,
    Turns(u8), // These represent 90 degree rotations clockwise.
}

impl TabOrientation {
    pub fn rotate_and_box<W: Widget<T> + 'static, T: Data>(self, widget: W, axis: Axis, cross: CrossAxisAlignment) ->Box<dyn Widget<T>>{
        let turns = match self {
            Self::Standard => match (axis, cross){
                (Axis::Horizontal, _)=>0,
                (Axis::Vertical, CrossAxisAlignment::Start)=>3,
                (Axis::Vertical, _ )=>1
            }
            Self::Turns(turns) => turns,
        };

        if turns == 0 {
            Box::new(widget)
        } else {
            Box::new( widget.rotate(turns) )
        }
    }
}

pub struct InitialTab<T> {
    name: String,
    child: TabBodyPod<T>,
}

enum TabsContent<T: Data> {
    StaticBuilder { tabs: Vec<InitialTab<T>> },
    // Dynamic { tabs_from_data: TabsFromData<T> }
    Running { scope: WidgetPod<T, TabsScope<T>> },
}

pub struct Tabs<T: Data> {
    axis: Axis,
    cross: CrossAxisAlignment, // Not sure if this should have another enum. Middle means nothing here
    rotation: TabOrientation,
    content: TabsContent<T>,
}

impl<T: Data> Tabs<T> {
    pub fn new() -> Self {
        Tabs {
            axis: Axis::Horizontal,
            cross: CrossAxisAlignment::Start,
            rotation: TabOrientation::Standard,
            content: TabsContent::StaticBuilder { tabs: Vec::new() },
        }
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

    pub fn with_tab(mut self, name: impl Into<String>, child: impl Widget<T> + 'static) -> Tabs<T> {
        self.add_tab(name, child);
        self
    }

    pub fn add_tab(&mut self, name: impl Into<String>, child: impl Widget<T> + 'static) {
        if let StaticBuilder { tabs } = &mut self.content {
            let tab = InitialTab {
                name: name.into(),
                child: WidgetPod::new(Box::new(child)),
            };
            tabs.push(tab)
        }
    }
}

impl<T: Data> Widget<T> for Tabs<T> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        if let Running { scope } = &mut self.content {
            scope.event(ctx, event, data, env);
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        if let LifeCycle::WidgetAdded = event {
            if let StaticBuilder { tabs } = &mut self.content {
                let mut body = TabsBody::new(self.axis);
                let mut names = Vector::new();

                for tab in tabs.drain(0..) {
                    names.push_back(tab.name);
                    body.add_pod(tab.child);
                }

                let (bar, body) = (
                    (TabBar::new(self.axis, self.cross, self.rotation), 0.0),
                    (
                        body.padding(5.).border(theme::BORDER_DARK, 0.5).expand(),
                        1.0,
                    ),
                );
                let mut layout: Flex<TabsState<T>> = Flex::for_axis(self.axis.cross());

                if let CrossAxisAlignment::End = self.cross {
                    layout.add_flex_child(body.0, body.1);
                    layout.add_flex_child(bar.0, bar.1);
                } else {
                    layout.add_flex_child(bar.0, bar.1);
                    layout.add_flex_child(body.0, body.1);
                };

                self.content = Running {
                    scope: WidgetPod::new(Scope::new(
                        TabsScopePolicy::new(names, 0),
                        Box::new(layout),
                    )),
                };
                ctx.children_changed();
            }
        }
        if let Running { scope } = &mut self.content {
            scope.lifecycle(ctx, event, data, env)
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        if let Running { scope } = &mut self.content {
            scope.update(ctx, data, env);
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        if let Running { scope } = &mut self.content {
            let size = scope.layout(ctx, bc, data, env);
            scope.set_layout_rect(ctx, data, env, Rect::from_origin_size(Point::ORIGIN, size));
            size
        } else {
            bc.min()
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        if let Running { scope } = &mut self.content {
            scope.paint(ctx, data, env)
        }
    }
}
