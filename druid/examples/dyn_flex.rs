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

use druid::im::Vector;
use druid::lens::Index;
use druid::widget::prelude::*;
use druid::widget::{
    Axis, Button, Checkbox, ConditionalContent, CrossAxisAlignment, Flex, ForEachContent, Label,
    MainAxisAlignment, StaticContent, Stepper, Tabs,
};
use druid::{AppLauncher, Color, Data, Lens, LensExt, WidgetExt, WindowDesc};

#[derive(Clone, Data, Lens, Debug)]
struct DynFlexState {
    show_header: bool,
    max: usize,
    items: Vector<String>,
}

pub fn main() {
    let main_window = WindowDesc::new(build_root_widget)
        .title("Dynamic flex!")
        .window_size((400.0, 600.0));
    let initial_state: DynFlexState = DynFlexState {
        show_header: true,
        max: 5,
        items: ["Apple", "Orange", "Grape"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    };
    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(initial_state)
        .expect("Failed to launch application");
}

fn build_root_widget() -> impl Widget<DynFlexState> {
    Tabs::new()
        .with_tab("List", list())
        .with_tab("Reorder", list_reorder())
}

fn list() -> impl Widget<DynFlexState> {
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
                    ),
                ) + ForEachContent::new(
                    |hs: &DynFlexState, _| 1..=hs.max,
                    |_, _, num| {
                        Label::new(format!("Label {}", num))
                            .expand()
                            .border(Color::WHITE, 1.0)
                            .flex(1. / ((num % 3 + 1) as f64))
                    },
                ) + Label::new("Footer")
                    .expand()
                    .border(Color::WHITE, 1.0)
                    .flex(0.5)
                    .content(),
            )
            .fix_size(250.0, 500.0)
            .border(Color::WHITE, 2.0),
        )
}

fn list_reorder() -> impl Widget<DynFlexState> {
    Flex::for_axis_content(
        Axis::Vertical,
        ForEachContent::new(
            |hs: &DynFlexState, _| 0..hs.items.len(),
            |_, _, idx| {
                Flex::for_axis_content(
                    Axis::Horizontal,
                    Button::new("Up")
                        .on_click(move |_, items: &mut Vector<String>, _| {
                            if idx > 0 {
                                items.swap(idx, idx - 1)
                            }
                        })
                        .content()
                        + Button::new("Down")
                            .on_click(move |_, items: &mut Vector<String>, _| {
                                if idx < items.len() - 1 {
                                    items.swap(idx, idx + 1)
                                }
                            })
                            .content()
                        + Label::new(move |hs: &String, _: &Env| format!("Item {} {}", idx, hs))
                            .lens(Index::new(idx))
                            .content(),
                )
                .main_axis_alignment(MainAxisAlignment::Start)
                .fix_height(40.)
                .lens(DynFlexState::items)
            },
        ),
    )
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .fix_size(250.0, 500.0)
    .border(Color::WHITE, 2.0)
}
