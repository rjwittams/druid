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

use druid::widget::{Align, Button, Flex, Label, SubWindowRequirement, TextBox};
use druid::{
    AppLauncher, Data, Env, Lens, LensExt, LocalizedString, Point, Size, Widget, WidgetExt,
    WindowConfig, WindowDesc,
};

const VERTICAL_WIDGET_SPACING: f64 = 20.0;
const TEXT_BOX_WIDTH: f64 = 200.0;
const WINDOW_TITLE: LocalizedString<HelloState> = LocalizedString::new("Hello World!");

#[derive(Clone, Data, Lens)]
struct SubState {
    my_stuff: String,
}

#[derive(Clone, Data, Lens)]
struct HelloState {
    name: String,
    sub: SubState,
}

pub fn main() {
    // describe the main window
    let main_window = WindowDesc::new(build_root_widget)
        .title(WINDOW_TITLE)
        .window_size((400.0, 400.0));

    // create the initial app state
    let initial_state = HelloState {
        name: "World".into(),
        sub: SubState {
            my_stuff: "It's mine!".into(),
        },
    };

    // start the application
    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(initial_state)
        .expect("Failed to launch application");
}

fn build_root_widget() -> impl Widget<HelloState> {
    // a label that will determine its text based on the current app data.
    let label = Label::new(|data: &HelloState, _env: &Env| {
        format!("Hello {}! {} ", data.name, data.sub.my_stuff)
    });
    // a textbox that modifies `name`.
    let textbox = TextBox::new()
        .with_placeholder("Who are we greeting?")
        .fix_width(TEXT_BOX_WIDTH)
        .lens(HelloState::sub.then(SubState::my_stuff));

    let button = Button::new("Make sub window")
        .on_click(|ctx, data: &mut SubState, _env| {
            let req = SubWindowRequirement::new(
                ctx.widget_id(),
                WindowConfig::new()
                    //.show_titlebar(false)
                    .window_size(Size::new(100., 100.))
                    .set_position(Point::new(1000.0, 500.0)),
                TextBox::new().lens(SubState::my_stuff),
                data.clone(),
            );

            ctx.new_sub_window(req)
        })
        .center()
        .lens(HelloState::sub);

    // arrange the two widgets vertically, with some padding
    let layout = Flex::column()
        .with_child(label)
        .with_spacer(VERTICAL_WIDGET_SPACING)
        .with_child(textbox)
        .with_child(button);

    // center the two widgets in the available space
    Align::centered(layout)
}
