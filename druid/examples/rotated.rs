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

use druid::widget::{Flex, Label, Stepper, TextBox, ViewSwitcher};
use druid::{
    AppLauncher, Data, Env, FontDescriptor, FontFamily, Lens, LocalizedString, UnitPoint, Widget,
    WidgetExt, WindowDesc,
};

const VERTICAL_WIDGET_SPACING: f64 = 20.0;
const TEXT_BOX_WIDTH: f64 = 200.0;
const WINDOW_TITLE: LocalizedString<RotatedState> = LocalizedString::new("Hello there!");

#[derive(Clone, Data, Lens)]
struct RotatedState {
    turns: f64,
    name: String,
}

pub fn main() {
    // describe the main window
    let main_window = WindowDesc::new(build_root_widget)
        .title(WINDOW_TITLE)
        .window_size((400.0, 400.0));

    // create the initial app state
    let initial_state = RotatedState {
        turns: 1.,
        name: "World".into(),
    };

    // start the application
    AppLauncher::with_window(main_window)
        .launch(initial_state)
        .expect("Failed to launch application");
}

fn build_root_widget() -> impl Widget<RotatedState> {
    // arrange the two widgets vertically, with some padding
    let rotated_part = ViewSwitcher::new(
        |rs: &RotatedState, _: &Env| rs.turns as u8,
        |turns: &u8, _: &RotatedState, _: &Env| {
            // a label that will determine its text based on the current app data.
            let label = Label::new(|data: &RotatedState, _env: &Env| {
                if data.name.is_empty() {
                    "Hello anybody!?".to_string()
                } else {
                    format!("Hello {}!", data.name)
                }
            })
            .with_font(FontDescriptor::new(FontFamily::SERIF).with_size(32.0))
            .align_horizontal(UnitPoint::CENTER);

            // a textbox that modifies `name`.
            let textbox = TextBox::new()
                .with_placeholder("Who are we greeting?")
                .with_text_size(18.0)
                .fix_width(TEXT_BOX_WIDTH)
                .align_horizontal(UnitPoint::CENTER)
                .lens(RotatedState::name);

            Flex::column()
                .with_child(label)
                .with_spacer(VERTICAL_WIDGET_SPACING)
                .with_child(textbox)
                .align_vertical(UnitPoint::CENTER)
                .rotate(*turns)
                .boxed()
        },
    );

    let row = Flex::row()
        .with_flex_spacer(0.5)
        .with_child(Label::new(|data: &f64, _env: &_| {
            format!("Quarter turns: {}", *data as u8)
        }))
        .with_child(Stepper::new().with_range(0., 255.))
        .with_flex_spacer(0.5)
        .lens(RotatedState::turns);

    Flex::column()
        .with_child(row)
        .with_spacer(VERTICAL_WIDGET_SPACING)
        .with_child(rotated_part)
}
