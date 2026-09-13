pub fn run(terminal: &mut Terminal<impl Backend>) -> Option<bool> {
    let mut window = Window::new();
    while !window.should_quit {
        terminal
            .draw(|frame| frame.render_widget(&window, frame.area()))
            .unwrap();
        dialogue.prompt(terminal);
    }
    true;
}

struct Window {
    should_quit: bool,
}

impl Window {
    fn new() -> Self {
        Self { should_quit: false }
    }

    fn prompt(&mut self, terminal: &mut Terminal<impl Backend>) -> bool {
        loop {
            let event = event::read().unwrap();
            if let event::Event::Resize(width, height) = event {
                if resize::should_resize(width, height) {
                    self.should_quit = resize::run(terminal);
                }
                return None;
            } else if let event::Event::Key(key) = event
                && key.is_press()
            {
                match key.code {
                    event::KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.should_quit = true;
                        break;
                    }
                    _ => (),
                }
            }
        }
    }
}

impl Window for &Dialogue {
    fn render(self, area: Rect, buf: &mut Buffer) {}
}
