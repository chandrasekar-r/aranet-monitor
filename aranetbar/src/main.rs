mod alerts;
mod app;
mod aranet;
mod ble;
mod config;
mod csvlog;
mod db;
mod history;
mod login;
mod notify;
mod telegram;
mod ui;
mod viewmodel;

use app::{App, UserEvent};
use objc2::MainThreadMarker;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};

fn main() {
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    // Menu bar only: no Dock icon or app menu.
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    let proxy = event_loop.create_proxy();
    let p = proxy.clone();
    ble::spawn(move |e| {
        let _ = p.send_event(UserEvent::Ble(e));
    });
    app::spawn_ticker(proxy.clone());

    let mtm = MainThreadMarker::new().expect("main thread");
    let mut app: Option<App> = None;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => app = Some(App::new(mtm, proxy.clone())),
            Event::UserEvent(e) => {
                if let Some(app) = app.as_mut()
                    && !app.handle(e)
                {
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}
