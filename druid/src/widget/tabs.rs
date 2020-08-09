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

use crate::theme;
use crate::widget::{Button, Flex, LabelText, MainAxisAlignment, Scope};
use crate::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx,
    Point, Rect, Size, UpdateCtx, Widget, WidgetExt, WidgetPod,
};

#[derive(Data, Clone)]
pub struct TabState<T: Data> {
    pub selected: usize,
    pub inner: T,
}

impl<T: Data> TabState<T> {
    pub fn new(inner: T) -> Self {
        TabState { selected: 0, inner }
    }
}

type Pod<T> = WidgetPod<T, Box<dyn Widget<T>>>;

pub struct TabBody<T> {
    children: Vec<Pod<T>>,
}

impl<T> TabBody<T> {
    pub fn empty() -> TabBody<T> {
        TabBody { children: vec![] }
    }

    pub fn new(child: impl Widget<T> + 'static) -> TabBody<T> {
        Self::empty().with_child(child)
    }

    pub fn with_child(mut self, child: impl Widget<T> + 'static) -> TabBody<T> {
        self.add_child(child);
        self
    }

    pub fn add_child(&mut self, child: impl Widget<T> + 'static) {
        self.children.push(WidgetPod::new(Box::new(child)));
    }
}

impl<T: Data> TabBody<T> {
    fn active(&mut self, state: &TabState<T>) -> Option<&mut Pod<T>> {
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

impl<T: Data> Widget<TabState<T>> for TabBody<T> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut TabState<T>, env: &Env) {
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
        data: &TabState<T>,
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
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        _old_data: &TabState<T>,
        data: &TabState<T>,
        env: &Env,
    ) {
        if _old_data.selected != data.selected {
            ctx.request_layout();
        }
        for child in &mut self.children {
            child.update(ctx, &data.inner, env);
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &TabState<T>,
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

    fn paint(&mut self, ctx: &mut PaintCtx, data: &TabState<T>, env: &Env) {
        if let Some(ref mut child) = self.active(data) {
            child.paint_raw(ctx, &data.inner, env);
        }
    }
}

pub struct TabBuilder<T: Data> {
    bar: Flex<usize>,
    body: TabBody<T>,
}

impl <T: Data> Default for TabBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Data> TabBuilder<T> {
    pub fn new() -> Self {
        TabBuilder {
            bar: Flex::row().main_axis_alignment(MainAxisAlignment::Start),
            body: TabBody::empty(),
        }
    }

    pub fn with_tab(
        mut self,
        name: impl Into<LabelText<usize>>,
        child: impl Widget<T> + 'static,
    ) -> TabBuilder<T> {
        self.add_tab(name, child);
        self
    }

    pub fn add_tab(&mut self, name: impl Into<LabelText<usize>>, child: impl Widget<T> + 'static) {
        let idx = self.body.children.len();
        self.bar.add_child(
            Button::new(name).on_click(move |_ctx, selected: &mut usize, _env| *selected = idx),
        );
        self.body.add_child(child);
    }

    pub fn build(self) -> impl Widget<T> {
        let layout = Flex::column()
            .with_child(
                self.bar
                    .with_flex_spacer(1.)
                    .lens(lens!(TabState<T>, selected)),
            )
            .with_flex_child(
                self.body
                    .padding(5.)
                    .border(theme::BORDER_LIGHT, 0.5)
                    .expand(),
                1.0,
            );
        Tabs {
            scope: WidgetPod::new(Box::new(Scope::new(TabState::new, lens!(TabState<T>, inner), layout)))
        }
    }
}

pub struct Tabs<T> {
    scope: WidgetPod<T, Box<dyn Widget<T>>>,
}

impl<T: Data> Widget<T> for Tabs<T> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        self.scope.event(ctx, event, data, env);
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        self.scope.lifecycle(ctx, event, data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        self.scope.update(ctx, data, env);
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        let size = self.scope.layout(ctx, bc, data, env);
        self.scope
            .set_layout_rect(ctx, data, env, Rect::from_origin_size(Point::ORIGIN, size));
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        self.scope.paint(ctx, data, env)
    }
}
