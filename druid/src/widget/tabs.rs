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
use crate::widget::{Button, Flex, LabelText, MainAxisAlignment, Scope, ScopePolicy, Label};
use crate::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx,
    Point, Rect, Size, UpdateCtx, Widget, WidgetExt, WidgetPod,
};
use std::marker::PhantomData;

type TabsScope<T> = Scope<TabsScopePolicy<T>, Box<dyn Widget<TabsState<T>>>>;
type TabBodyPod<T> = WidgetPod<T, Box<dyn Widget<T>>>;
type TabIndex = usize;


#[derive(Data, Clone)]
pub struct TabsState<T: Data> {
    pub selected: TabIndex,
    pub inner: T,
}

impl<T: Data> TabsState<T> {
    pub fn new(inner: T) -> Self {
        TabsState { selected: 0, inner }
    }
}



pub struct TabsBody<T> {
    children: Vec<TabBodyPod<T>>,
}

impl<T> TabsBody<T> {
    pub fn empty() -> TabsBody<T> {
        TabsBody { children: vec![] }
    }

    pub fn new(child: impl Widget<T> + 'static) -> TabsBody<T> {
        Self::empty().with_child(child)
    }

    pub fn with_child(mut self, child: impl Widget<T> + 'static) -> TabsBody<T> {
        self.add_child(child);
        self
    }

    pub fn add_child(&mut self, child: impl Widget<T> + 'static) {
        self.add_pod(WidgetPod::new(Box::new(child)));
    }

    pub fn add_pod(&mut self, pod: TabBodyPod<T>){
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
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        _old_data: &TabsState<T>,
        data: &TabsState<T>,
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
        if let Some(ref mut child) = self.active(data) {
            child.paint_raw(ctx, &data.inner, env);
        }
    }
}



pub struct TabsScopePolicy<T>{
    phantom_t: PhantomData<T>
}

impl<T> TabsScopePolicy<T> {
    pub fn new() -> Self {
        TabsScopePolicy { phantom_t: Default::default() }
    }
}

// Would be easy to generate with a proc macro
impl <T: Data> ScopePolicy for TabsScopePolicy<T>{
    type In = T;
    type State = TabsState<T>;

    fn default_state(&self, inner: &Self::In) -> Self::State {
        TabsState::new(inner.clone())
    }

    fn replace_in_state(&self, state: &mut Self::State, inner: &Self::In) {
        state.inner = inner.clone();
    }

    fn write_back_input(&self, state: &Self::State, inner: &mut Self::In) {
        *inner = state.inner.clone();
    }
}

pub struct InitialTab<T>{
    name: LabelText<usize>,
    child: TabBodyPod<T>
}

pub struct Tabs<T: Data> {
    scope: Option<WidgetPod<T, TabsScope<T>>>,
    initial_tabs: Option<Vec<InitialTab<T>>>
}

impl<T: Data> Tabs<T> {
    pub fn new() -> Self {
        Tabs { scope: None, initial_tabs: Some(Vec::new()) }
    }
}

impl <T: Data> Tabs<T>{

    pub fn with_tab(
        mut self,
        name: impl Into<LabelText<usize>>,
        child: impl Widget<T> + 'static,
    ) -> Tabs<T> {
        self.add_tab(name, child);
        self
    }

    pub fn add_tab(&mut self, name: impl Into<LabelText<usize>>, child: impl Widget<T> + 'static) {
        if let Some(tabs) = &mut self.initial_tabs {
            let tab = InitialTab {
                name: name.into(),
                child: WidgetPod::new(Box::new(child))
            };
            tabs.push(tab)
        }
    }
}

impl<T: Data> Widget<T> for Tabs<T> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        if let Some(scope ) = &mut self.scope {
            scope.event(ctx, event, data, env);
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        eprintln!("lifecycle {:?} {} {}", event, self.scope.is_some(), self.initial_tabs.is_some());
        if let LifeCycle::WidgetAdded = event {
            if let Some(initial_tabs) = self.initial_tabs.take() {
                let mut bar = Flex::row().main_axis_alignment(MainAxisAlignment::Start);
                let mut body = TabsBody::empty();
                for (idx, tab) in initial_tabs.into_iter().enumerate() {
                    bar.add_child(
                        Button::new(tab.name).on_click(move |_ctx, selected: &mut usize, _env| *selected = idx),
                    );
                    body.add_pod(tab.child);
                }

                let layout :Flex<TabsState<T>> = Flex::column()
                    .with_child(
                        bar
                            .with_flex_spacer(1.)
                            .lens(lens!(TabsState<T>, selected)),
                    )
                    .with_flex_child(
                        body
                            .padding(5.)
                            .border(theme::BORDER_LIGHT, 0.5)
                            .expand(),
                        1.0,
                    );
                self.scope = Some(WidgetPod::new(Scope::new(TabsScopePolicy::new(), Box::new(layout))));
                eprintln!("Scope made for tabs");
            }
        }else if let Some(scope ) = &mut self.scope {
            eprintln!("lifecycle to scope {:?}", event);
            scope.lifecycle(ctx, event, data, env)
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        if let Some(scope) = &mut self.scope {
            scope.update(ctx, data, env);
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        if let Some(scope ) = &mut self.scope {
            let size = scope.layout(ctx, bc, data, env);
            scope.set_layout_rect(ctx, data, env, Rect::from_origin_size(Point::ORIGIN, size));
            size
        }else {
            bc.min()
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        if let Some(scope ) = &mut self.scope {
            scope.paint(ctx, data, env)
        }
    }
}
