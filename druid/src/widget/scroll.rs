// Copyright 2019 The Druid Authors.
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

//! A container that scrolls its contents.

use std::f64::INFINITY;

use crate::widget::prelude::*;
use crate::widget::{Axis, BindableProperty, Bindable, ClipBox};
use crate::{scroll_component::*, Data, Rect, Vec2, WidgetPod};
use std::marker::PhantomData;

#[derive(Debug, Clone)]
enum ScrollDirection {
    Bidirectional,
    Vertical,
    Horizontal,
}

/// A container that scrolls its contents.
///
/// This container holds a single child, and uses the wheel to scroll it
/// when the child's bounds are larger than the viewport.
///
/// The child is laid out with completely unconstrained layout bounds by
/// default. Restrict to a specific axis with [`vertical`] or [`horizontal`].
/// When restricted to scrolling on a specific axis the child's size is
/// locked on the opposite axis.
///
/// [`vertical`]: struct.Scroll.html#method.vertical
/// [`horizontal`]: struct.Scroll.html#method.horizontal
pub struct Scroll<T, W> {
    clip: ClipBox<T, W>,
    scroll_component: ScrollComponent,
    direction: ScrollDirection,
}

impl<T, W: Widget<T>> Scroll<T, W> {
    /// Create a new scroll container.
    ///
    /// This method will allow scrolling in all directions if child's bounds
    /// are larger than the viewport. Use [vertical](#method.vertical) and
    /// [horizontal](#method.horizontal) methods to limit scrolling to a specific axis.
    pub fn new(child: W) -> Scroll<T, W> {
        Scroll {
            clip: ClipBox::new(child),
            scroll_component: ScrollComponent::new(),
            direction: ScrollDirection::Bidirectional,
        }
    }

    /// Restrict scrolling to the vertical axis while locking child width.
    pub fn vertical(mut self) -> Self {
        self.direction = ScrollDirection::Vertical;
        self.scroll_component.scrollbars_enabled = ScrollbarsEnabled::Vertical;
        self.clip.set_constrain_vertical(false);
        self.clip.set_constrain_horizontal(true);
        self
    }

    /// Restrict scrolling to the horizontal axis while locking child height.
    pub fn horizontal(mut self) -> Self {
        self.direction = ScrollDirection::Horizontal;
        self.scroll_component.scrollbars_enabled = ScrollbarsEnabled::Horizontal;
        self.clip.set_constrain_vertical(true);
        self.clip.set_constrain_horizontal(false);
        self
    }

    pub fn disable_scrollbars(mut self) -> Self {
        self.scroll_component.scrollbars_enabled = ScrollbarsEnabled::None;
        self
    }

    pub fn only_vertical_scrollbar(mut self) -> Self {
        self.scroll_component.scrollbars_enabled = ScrollbarsEnabled::Vertical;
        self
    }

    pub fn only_horizontal_scrollbar(mut self) -> Self {
        self.scroll_component.scrollbars_enabled = ScrollbarsEnabled::Horizontal;
        self
    }

    /// Returns a reference to the child widget.
    pub fn child(&self) -> &W {
        self.clip.child()
    }

    /// Returns a mutable reference to the child widget.
    pub fn child_mut(&mut self) -> &mut W {
        self.clip.child_mut()
    }

    /// Returns the size of the child widget.
    pub fn child_size(&self) -> Size {
        self.clip.content_size()
    }

    /// Returns the current scroll offset.
    pub fn offset(&self) -> Vec2 {
        self.clip.viewport_origin().to_vec2()
    }

    /// Scroll by `delta` units.
    ///
    /// Returns `true` if the scroll offset has changed.
    pub fn scroll_by(&mut self, delta: Vec2) -> bool {
        self.clip.pan_by(delta)
    }

    /// Scroll the minimal distance to show the target rect.
    ///
    /// If the target region is larger than the viewport, we will display the
    /// portion that fits, prioritizing the portion closest to the origin.
    pub fn scroll_to(&mut self, region: Rect) -> bool {
        self.clip.pan_to_visible(region)
    }

    /// Scroll to this position on a particular axis.
    ///
    /// Returns `true` if the scroll offset has changed.
    pub fn scroll_to_direction(&mut self, axis: Axis, position: f64, size: Size) -> bool {
        let scrolled = self.scroll_component.scroll_on_axis(axis, position, size);
        self.child.set_viewport_offset(self.offset());
        scrolled
    }

    /// Return the scroll offset on a particular axis
    pub fn offset_for_axis(&self, axis: Axis) -> f64 {
        self.scroll_component.offset_for_axis(axis)
    }
}

impl<T: Data, W: Widget<T>> Widget<T> for Scroll<T, W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        let scroll_component = &mut self.scroll_component;
        self.clip.with_port(|port| {
            scroll_component.event(port, ctx, event, env);
        });
        if !ctx.is_handled() {
            self.clip.event(ctx, event, data, env);
        }

        self.clip.with_port(|port| {
            scroll_component.handle_scroll(port, ctx, event, env);
        });
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        self.scroll_component.lifecycle(ctx, event, env);
        self.clip.lifecycle(ctx, event, data, env);
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &T, data: &T, env: &Env) {
        self.clip.update(ctx, old_data, data, env);
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        bc.debug_check("Scroll");

        let old_size = self.clip.viewport().rect.size();
        let child_size = self.clip.layout(ctx, &bc, data, env);
        log_size_warnings(child_size);

        let self_size = bc.constrain(child_size);
        // The new size might have made the current scroll offset invalid. This makes it valid
        // again.
        let _ = self.scroll_by(Vec2::ZERO);
        if old_size != self_size {
            self.scroll_component
                .reset_scrollbar_fade(|d| ctx.request_timer(d), env);
        }

        self_size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        self.clip.paint(ctx, data, env);
        self.scroll_component
            .draw_bars(ctx, &self.clip.viewport(), env);
    }
}

fn log_size_warnings(size: Size) {
    if size.width.is_infinite() {
        log::warn!("Scroll widget's child has an infinite width.");
    }

    if size.height.is_infinite() {
        log::warn!("Scroll widget's child has an infinite height.");
    }
}

pub struct ScrollToProperty<T, W> {
    direction: Axis, // We have no public direction/axis type, but two private ones. Sigh.
    phantom_t: PhantomData<T>,
    phantom_w: PhantomData<W>,
}

impl<T, W> ScrollToProperty<T, W> {
    pub fn new(direction: Axis) -> Self {
        ScrollToProperty {
            direction,
            phantom_t: Default::default(),
            phantom_w: Default::default(),
        }
    }
}

impl<T, W: Widget<T>> BindableProperty for ScrollToProperty<T, W> {
    type Controlling = Scroll<T, W>;
    type Value = f64;
    type Change = ();

    fn write_prop(
        &self,
        controlled: &mut Self::Controlling,
        ctx: &mut UpdateCtx,
        position: &Self::Value,
        _env: &Env,
    ) {
        controlled.scroll_to_direction(self.direction, *position, ctx.size());
        ctx.request_paint()
    }

    fn append_changes(
        &self,
        controlled: &Self::Controlling,
        field_val: &Self::Value,
        change: &mut Option<Self::Change>,
        _env: &Env,
    ) {
        if !controlled.offset_for_axis(self.direction).same(field_val) {
            *change = Some(())
        }
    }

    fn update_data_from_change(
        &self,
        controlled: &Self::Controlling,
        _ctx: &EventCtx,
        field: &mut Self::Value,
        _change: Self::Change,
        _env: &Env,
    ) {
        *field = controlled.offset_for_axis(self.direction)
    }
}

impl<T, W> Bindable for Scroll<T, W> {}
