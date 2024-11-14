use std::fs;
use source_viewer::walk::*;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = create_terminal()?;
    let _cleanup = TerminalCleanup;
    let mut state = State::new();

    state.list_state.select(Some(0)); // Initialize the selected index

    loop {
        match state.mode {
            Mode::Dir => {
                let entries: Vec<_> = fs::read_dir(&state.current_dir)?
                    .filter_map(Result::ok)
                    .collect();
                render_directory(&mut terminal, &entries, &mut state)?;
                if handle_directory_input(&entries, &mut state)? {
                    break;
                }
            }
            Mode::File => {
                render_file_viewer(&mut terminal, &state)?;
                if handle_file_input(&mut state)? {
                    break;
                }
            }
        };
    }

    Ok(())
}
