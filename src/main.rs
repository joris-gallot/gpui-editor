use gpui::{
  App, Application, Bounds, Context, Entity, FocusHandle, Focusable, KeyBinding, Window,
  WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};

mod buffer;
mod document;
mod editor;
mod editor_element;

// Initial window size
const INITIAL_WINDOW_WIDTH: f32 = 1200.0;
const INITIAL_WINDOW_HEIGHT: f32 = 800.0;

use editor::{
  AltLeft, AltRight, Backspace, BackspaceAll, BackspaceWord, CmdDown, CmdLeft, CmdRight, CmdUp,
  Copy, Cut, Delete, Down, Editor, End, Enter, Home, Left, Paste, Quit, Right, SelectAll,
  SelectCmdDown, SelectCmdLeft, SelectCmdRight, SelectCmdUp, SelectDown, SelectLeft, SelectRight,
  SelectUp, SelectWordLeft, SelectWordRight, ShowCharacterPalette, Up,
};

struct EditorExample {
  editor: Entity<Editor>,
  focus_handle: FocusHandle,
}

impl Focusable for EditorExample {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for EditorExample {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .bg(rgb(0xaaaaaa))
      .track_focus(&self.focus_handle(cx))
      .flex()
      .flex_col()
      .size_full()
      .child(self.editor.clone())
  }
}

fn main() {
  Application::new().run(|cx: &mut App| {
    let bounds = Bounds::centered(
      None,
      size(px(INITIAL_WINDOW_WIDTH), px(INITIAL_WINDOW_HEIGHT)),
      cx,
    );

    cx.bind_keys([
      KeyBinding::new("enter", Enter, None),
      KeyBinding::new("backspace", Backspace, None),
      KeyBinding::new("alt-backspace", BackspaceWord, None),
      KeyBinding::new("cmd-backspace", BackspaceAll, None),
      KeyBinding::new("delete", Delete, None),
      KeyBinding::new("up", Up, None),
      KeyBinding::new("down", Down, None),
      KeyBinding::new("left", Left, None),
      KeyBinding::new("alt-left", AltLeft, None),
      KeyBinding::new("cmd-left", CmdLeft, None),
      KeyBinding::new("right", Right, None),
      KeyBinding::new("alt-right", AltRight, None),
      KeyBinding::new("cmd-right", CmdRight, None),
      KeyBinding::new("cmd-up", CmdUp, None),
      KeyBinding::new("cmd-down", CmdDown, None),
      KeyBinding::new("shift-up", SelectUp, None),
      KeyBinding::new("shift-down", SelectDown, None),
      KeyBinding::new("shift-cmd-left", SelectCmdLeft, None),
      KeyBinding::new("shift-cmd-right", SelectCmdRight, None),
      KeyBinding::new("shift-cmd-up", SelectCmdUp, None),
      KeyBinding::new("shift-cmd-down", SelectCmdDown, None),
      KeyBinding::new("shift-left", SelectLeft, None),
      KeyBinding::new("shift-alt-left", SelectWordLeft, None),
      KeyBinding::new("shift-right", SelectRight, None),
      KeyBinding::new("shift-alt-right", SelectWordRight, None),
      KeyBinding::new("cmd-a", SelectAll, None),
      KeyBinding::new("cmd-v", Paste, None),
      KeyBinding::new("cmd-c", Copy, None),
      KeyBinding::new("cmd-x", Cut, None),
      KeyBinding::new("home", Home, None),
      KeyBinding::new("end", End, None),
      KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, None),
    ]);

    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..Default::default()
        },
        |_, cx| {
          cx.new(|cx| EditorExample {
            editor: cx.new(Editor::new),
            focus_handle: cx.focus_handle(),
          })
        },
      )
      .unwrap();

    window
      .update(cx, |view, window, cx| {
        window.focus(&view.editor.focus_handle(cx));
        cx.activate(true);
      })
      .unwrap();

    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
  });
}
