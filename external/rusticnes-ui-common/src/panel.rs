use application::RuntimeState;
use drawing::SimpleBuffer;
use events::Event;

pub trait Panel {
    fn title(&self) -> &str;
    fn handle_event(&mut self, runtime: &RuntimeState, event: Event) -> Vec<Event>;
    fn active_canvas(&self) -> &SimpleBuffer;
    fn scale_factor(&self) -> u32 {return 1;}
    fn shown(&self) -> bool;
}