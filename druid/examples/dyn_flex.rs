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

use druid::widget::prelude::*;
use druid::widget::{
    Axis, Checkbox, ConditionalContent, Flex, ContentExt, ForEachContent, Label, StaticContent,
    Stepper,
};
use druid::{AppLauncher, Color, Data, Lens, LensExt, WidgetExt, WindowDesc};

#[derive(Clone, Data, Lens, Debug)]
struct DynFlexState {
    show_header: bool,
    max: usize,
}

pub fn main() {
    let main_window = WindowDesc::new(build_root_widget)
        .title("Dynamic flex!")
        .window_size((400.0, 600.0));
    let initial_state: DynFlexState = DynFlexState {
        show_header: true,
        max: 5,
    };
    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(initial_state)
        .expect("Failed to launch application");
}

fn build_root_widget() -> impl Widget<DynFlexState> {
    Flex::column()
        .with_child(
            Flex::row()
                .with_child(Label::new(|hs: &DynFlexState, _: &Env| {
                    format!("Children:{}", hs.max)
                }))
                .with_child(
                    Stepper::new()
                        .with_range(0., 20.)
                        .lens((DynFlexState::max).map(
                            |x: &usize| *x as f64,
                            |x: &mut usize, y: f64| *x = y as usize,
                        )),
                )
                .with_child(
                    Checkbox::new("Show Header")
                        .padding(5.)
                        .lens(DynFlexState::show_header),
                ),
        )
        .with_child(
            Flex::for_axis_content(
                Axis::Vertical,
                ConditionalContent::new_if(
                    |data: &DynFlexState, _| data.show_header,
                    StaticContent::of(
                            Label::new("Header")
                                .expand()
                                .border(Color::WHITE, 1.0)
                                .flex(0.5),
                        )
                )
                .then(ForEachContent::new(
                    |hs: &DynFlexState, _| 1..=hs.max,
                    |_, _, num| {
                        Label::new(format!("Label {}", num))
                            .expand()
                            .border(Color::WHITE, 1.0)
                            .flex(1. / ((*num % 3 + 1) as f64))
                    },
                ))
                .then(StaticContent::of(
                    Label::new("Footer")
                        .expand()
                        .border(Color::WHITE, 1.0)
                        .flex(0.5),
                )),
            )
            .fix_size(250.0, 500.0)
            .border(Color::WHITE, 2.0),
        )
}
