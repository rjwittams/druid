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

//! A slider widget.

use crate::kurbo::{Circle, Shape};
use crate::widget::prelude::*;
use super::Axis;
use crate::{theme, LinearGradient, Point, Rect, UnitPoint};

const TRACK_THICKNESS: f64 = 4.0;
const BORDER_WIDTH: f64 = 2.0;
const KNOB_STROKE_WIDTH: f64 = 2.0;

/// A slider, allowing interactive update of a numeric value.
///
/// This slider implements `Widget<f64>`, and works on values clamped
/// in the range `min..max`.
#[derive(Debug, Clone)]
pub struct Slider {
    axis: Axis,
    min: f64,
    max: f64,
    knob_pos: Point,
    knob_hovered: bool,
    main_offset: f64,
}

impl Default for Slider{
    fn default() -> Self {
        Slider::new()
    }
}

impl Slider {
    /// Create a new `Slider`, defaulting to horizontal.
    pub fn new() -> Slider {
        Slider::for_axis(Axis::Horizontal)
    }

    /// Create a new `Slider` for the given axis.
    pub fn for_axis(axis: Axis) -> Slider {
        Slider {
            axis,
            min: 0.,
            max: 1.,
            knob_pos: Default::default(),
            knob_hovered: Default::default(),
            main_offset: Default::default(),
        }
    }

    /// Builder-style method to set the range covered by this slider.
    ///
    /// The default range is `0.0..1.0`.
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max;
        self
    }
}

impl Slider {
    fn knob_hit_test(&self, knob_width: f64, mouse_pos: Point) -> bool {
        let knob_circle = Circle::new(self.knob_pos, knob_width / 2.);
        knob_circle.winding(mouse_pos) > 0
    }

    fn flip(&self, val: f64)->f64{
        match self.axis{
            Axis::Horizontal => val,
            Axis::Vertical => 1. - val
        }
    }

    fn calculate_value(&self, mouse_main: f64, knob_width: f64, slider_extent: f64) -> f64 {
        let amount_along = mouse_main + self.main_offset - knob_width / 2.;
        let total_amount = slider_extent - knob_width;
        let scalar = (amount_along / total_amount).clamp(0., 1.);
        self.min + self.flip(scalar) * (self.max - self.min)
    }

    fn normalize(&self, data: f64) -> f64 {
        let norm = (data.max(self.min).min(self.max) - self.min) / (self.max - self.min);
        self.flip(norm)
    }
}

impl Widget<f64> for Slider {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut f64, env: &Env) {
        let knob_size = env.get(theme::BASIC_WIDGET_HEIGHT);
        let slider_extent = self.axis.major(ctx.size());

        match event {
            Event::MouseDown(mouse) => {
                ctx.set_active(true);
                if self.knob_hit_test(knob_size, mouse.pos) {
                    self.main_offset = self.axis.major_pos(self.knob_pos) - self.axis.major_pos(mouse.pos)
                } else {
                    self.main_offset = 0.;
                    *data = self.calculate_value(self.axis.major_pos(mouse.pos), knob_size, slider_extent);
                }
                ctx.request_paint();
            }
            Event::MouseUp(mouse) => {
                if ctx.is_active() {
                    ctx.set_active(false);
                    *data = self.calculate_value(self.axis.major_pos(mouse.pos), knob_size, slider_extent);
                    ctx.request_paint();
                }
            }
            Event::MouseMove(mouse) => {
                if ctx.is_active() {
                    *data = self.calculate_value(self.axis.major_pos(mouse.pos), knob_size, slider_extent);
                    ctx.request_paint();
                }
                if ctx.is_hot() {
                    let knob_hover = self.knob_hit_test(knob_size, mouse.pos);
                    if knob_hover != self.knob_hovered {
                        self.knob_hovered = knob_hover;
                        ctx.request_paint();
                    }
                }
            }
            _ => (),
        }
    }

    fn lifecycle(&mut self, _ctx: &mut LifeCycleCtx, _event: &LifeCycle, _data: &f64, _env: &Env) {}

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &f64, _data: &f64, _env: &Env) {
        ctx.request_paint();
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, _data: &f64, env: &Env) -> Size {
        bc.debug_check("Slider");
        let minor = env.get(theme::BASIC_WIDGET_HEIGHT);
        let major = env.get(theme::WIDE_WIDGET_WIDTH);
        if let Axis::Horizontal = self.axis
        {
            let baseline_offset = (minor / 2.0) - TRACK_THICKNESS;
            ctx.set_baseline_offset(baseline_offset);
        }
        let size = bc.constrain(self.axis.pack(major, minor));
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &f64, env: &Env) {
        let clamped = self.normalize(*data);

        let rect = ctx.size().to_rect();
        let knob_size = env.get(theme::BASIC_WIDGET_HEIGHT);

        let axis = self.axis;
        //Paint the background
        let background_major = axis.major(rect.size()) - knob_size;
        let background_origin: Point = axis.pack(knob_size / 2., (knob_size - TRACK_THICKNESS) / 2.).into();
        let background_size: Size = axis.pack(background_major, TRACK_THICKNESS).into();
        let background_rect = Rect::from_origin_size(background_origin, background_size)
            .inset(-BORDER_WIDTH / 2.)
            .to_rounded_rect(2.);

        let (start, end) = match axis{
            Axis::Horizontal => (UnitPoint::TOP, UnitPoint::BOTTOM),
            Axis::Vertical => (UnitPoint::LEFT, UnitPoint::RIGHT)
        };

        let background_gradient = LinearGradient::new(
            start,
            end,
            (
                env.get(theme::BACKGROUND_LIGHT),
                env.get(theme::BACKGROUND_DARK),
            ),
        );

        ctx.stroke(background_rect, &env.get(theme::BORDER_DARK), BORDER_WIDTH);

        ctx.fill(background_rect, &background_gradient);

        //Get ready to paint the knob
        let is_active = ctx.is_active();
        let is_hovered = self.knob_hovered;

        let knob_position = (axis.major(rect.size()) - knob_size) * clamped + knob_size / 2.;
        self.knob_pos = axis.pack(knob_position, knob_size / 2.).into();
        let knob_circle = Circle::new(self.knob_pos, (knob_size - KNOB_STROKE_WIDTH) / 2.);

        let normal_knob_gradient = LinearGradient::new(
            start,
            end,
            (
                env.get(theme::FOREGROUND_LIGHT),
                env.get(theme::FOREGROUND_DARK),
            ),
        );
        let flipped_knob_gradient = LinearGradient::new(
            start,
            end,
            (
                env.get(theme::FOREGROUND_DARK),
                env.get(theme::FOREGROUND_LIGHT),
            ),
        );

        let knob_gradient = if is_active {
            flipped_knob_gradient
        } else {
            normal_knob_gradient
        };

        //Paint the border
        let border_color = if is_hovered || is_active {
            env.get(theme::FOREGROUND_LIGHT)
        } else {
            env.get(theme::FOREGROUND_DARK)
        };

        ctx.stroke(knob_circle, &border_color, KNOB_STROKE_WIDTH);

        //Actually paint the knob
        ctx.fill(knob_circle, &knob_gradient);
    }

    fn post_render(&mut self) {}
}
