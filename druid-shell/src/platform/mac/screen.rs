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

//! macOS Monitors and Screen information.

use crate::screen::Monitor;
use crate::kurbo::Size;
use cocoa::foundation::NSArray;
use cocoa::base::{id};
use objc::{class, msg_send, sel, sel_impl};
use kurbo::Rect;
use cocoa::appkit::NSScreen;

pub(crate) fn get_display_size() -> Size {
    //unsafe  {
        let rect = get_monitors().iter().fold( Rect::ZERO, |rect, monitor| rect.union( monitor.virtual_rect()));
        rect.size()
    //}
}

pub(crate) fn get_monitors() -> Vec<Monitor> {
    unsafe {
        let screens: id = msg_send![class![NSScreen], screens];
        let mut monitors = Vec::<Monitor>::new();
        let mut total_rect = Rect::ZERO;

        for idx in 0..screens.count() {
            let screen = screens.objectAtIndex(idx);
            let frame =  NSScreen::frame(screen);

            let frame_r = Rect::from_origin_size((frame.origin.x, frame.origin.y), (frame.size.width,  frame.size.height));
            let vis_frame = NSScreen::visibleFrame(screen);
            let vis_frame_r = Rect::from_origin_size(  (vis_frame.origin.x, vis_frame.origin.y), (vis_frame.size.width,  vis_frame.size.height) );
            monitors.push(Monitor::new(idx == 0,  frame_r,  vis_frame_r) );
            total_rect = total_rect.union(frame_r)
        }

        //Flip y coordinates of origins (On mac, Y goes up)



        monitors
    }
}
