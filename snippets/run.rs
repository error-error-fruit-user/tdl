pub fn run(terminal: &mut Terminal<impl Backend>, legend: &[[&'static str; 2]]) -> bool {
    loop {
        terminal.draw(|frame| {}).unwrap();
        loop {
            let event = event::read().unwrap();
            if let event::Event::Resize(width, height) = event {
                if resize::should_resize(width, height) {
                    return resize::run(terminal);
                }
                break;
            } else if let event::Event::Key(key) = event
                && key.is_press()
            {
                match key.code {
                    event::KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return true;
                    }
                    _ => (),
                }
            } else {
                return false;
            }
        }
    }
}
