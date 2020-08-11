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

use druid::widget::{Align, Flex, Label, TextBox, Button, SubWindowRequirement, SubWindowPort};
use druid::{AppLauncher, Data, Env, Lens, LensExt, LocalizedString, Widget, WidgetExt, WindowDesc, WidgetPod, LifeCycle, EventCtx, PaintCtx, LifeCycleCtx, BoxConstraints, Size, LayoutCtx, Event, UpdateCtx, Rect, Point, Color, RenderContext};

const VERTICAL_WIDGET_SPACING: f64 = 20.0;
const TEXT_BOX_WIDTH: f64 = 200.0;
const WINDOW_TITLE: LocalizedString<HelloState> = LocalizedString::new("Hello World!");

#[derive(Clone, Data, Lens)]
struct SubState{
    my_stuff: String
}

#[derive(Clone, Data, Lens)]
struct HelloState {
    name: String,
    sub: SubState
}

pub fn main() {
    // describe the main window
    let main_window = WindowDesc::new(build_root_widget)
        .title(WINDOW_TITLE)
        .window_size((400.0, 400.0));

    // create the initial app state
    let initial_state = HelloState {
        name: "World".into(),
        sub: SubState{
            my_stuff: "It's mine!".into()
        }
    };

    // start the application
    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(initial_state)
        .expect("Failed to launch application");
}

struct SubOwner{
    port_pod: Option<WidgetPod<SubState,  SubWindowPort<SubState>>>
}

impl SubOwner {
    pub fn new() -> Self {
        SubOwner { port_pod: None }
    }
}

impl Widget<SubState> for SubOwner{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut SubState, env: &Env) {
        match event{
            Event::MouseDown(e)=>{
                let desc = WindowDesc::new(|| TextBox::new().lens(SubState::my_stuff));
                let (req, port) = SubWindowRequirement::make_requirement_and_port( (*data).clone(), desc );
                self.port_pod = Some(WidgetPod::new(port));
                ctx.new_sub_window(req);
                ctx.set_handled();
                ctx.children_changed();
            }
            _=> if let Some(pod) = &mut self.port_pod {
                pod.event(ctx, event, data, env);
            }
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &SubState, env: &Env) {
        if let Some(pod) = &mut self.port_pod {
            pod.lifecycle(ctx, event, data, env);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &SubState, data: &SubState, env: &Env) {
        if let Some(pod) = &mut self.port_pod {
            pod.update(ctx, data, env);
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &SubState, env: &Env) -> Size {
        if let Some(pod) = &mut self.port_pod{
            let size = pod.layout(ctx, bc, data, env);
            pod.set_layout_rect(ctx, data, env, Rect::from_origin_size(Point::ORIGIN, size));
        }
        bc.constrain(Size::new(100.0, 100.0))
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &SubState, env: &Env) {
        let sz = ctx.size();
        ctx.fill( Rect::from_origin_size(Point::ZERO, sz), &Color::WHITE )
    }
}

fn build_root_widget() -> impl Widget<HelloState> {
    // a label that will determine its text based on the current app data.
    let label = Label::new(|data: &HelloState, _env: &Env| format!("Hello {}! {} ", data.name, data.sub.my_stuff));
    // a textbox that modifies `name`.
    let textbox = TextBox::new()
        .with_placeholder("Who are we greeting?")
        .fix_width(TEXT_BOX_WIDTH)
        .lens(HelloState::sub.then(SubState::my_stuff));

    let button = SubOwner::new().lens(HelloState::sub);

    // arrange the two widgets vertically, with some padding
    let layout = Flex::column()
        .with_child(label)
        .with_spacer(VERTICAL_WIDGET_SPACING)
        .with_child(textbox)
        .with_child( button);

    // center the two widgets in the available space
    Align::centered(layout)
}
