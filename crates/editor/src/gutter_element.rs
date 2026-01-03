use gpui::{
  App, Bounds, ElementId, Entity, GlobalElementId, InspectorElementId, LayoutId, Pixels, Style,
  TextAlign, TextRun, Window, point, prelude::*, px, relative, rgb,
};
use std::ops::Range;

use crate::editor::Editor;

pub struct GutterElement {
  editor: Entity<Editor>,
}

pub struct GutterPrepaintState {
  line_numbers: Vec<(usize, String)>,
  viewport: Range<usize>,
  line_height: Pixels,
  is_dark: bool,
}

impl GutterElement {
  pub fn new(editor: Entity<Editor>) -> Self {
    Self { editor }
  }
}

impl IntoElement for GutterElement {
  type Element = Self;

  fn into_element(self) -> Self::Element {
    self
  }
}

impl Element for GutterElement {
  type RequestLayoutState = ();
  type PrepaintState = GutterPrepaintState;

  fn id(&self) -> Option<ElementId> {
    None
  }

  fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
    None
  }

  fn request_layout(
    &mut self,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    window: &mut Window,
    cx: &mut App,
  ) -> (LayoutId, Self::RequestLayoutState) {
    let mut style = Style::default();
    style.size.width = relative(1.).into();
    style.size.height = relative(1.).into();

    (window.request_layout(style, [], cx), ())
  }

  fn prepaint(
    &mut self,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>,
    _request_layout: &mut Self::RequestLayoutState,
    window: &mut Window,
    cx: &mut App,
  ) -> Self::PrepaintState {
    let (viewport, line_numbers, line_height, is_dark) = {
      let editor = self.editor.read(cx);
      let document = editor.document().read(cx);
      let line_height = window.line_height();
      let scroll_offset = editor.scroll_offset_y;

      // Calculate viewport (same logic as EditorElement)
      let visible_line_count = ((bounds.size.height / line_height).ceil() as usize).max(1);
      let start_line = (scroll_offset.floor() as usize).min(document.len_lines().saturating_sub(1));
      let end_line = (start_line + visible_line_count).min(document.len_lines());
      let viewport = start_line..end_line;

      // Format line numbers for visible lines
      let mut line_numbers = Vec::new();
      for line_idx in viewport.clone() {
        let line_number = format!("{}", line_idx + 1);
        line_numbers.push((line_idx, line_number));
      }

      (viewport, line_numbers, line_height, editor.theme.is_dark)
    };

    GutterPrepaintState {
      line_numbers,
      viewport,
      line_height,
      is_dark,
    }
  }

  fn paint(
    &mut self,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>,
    _request_layout: &mut Self::RequestLayoutState,
    prepaint: &mut Self::PrepaintState,
    window: &mut Window,
    cx: &mut App,
  ) {
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    let text_color = if prepaint.is_dark {
      rgb(0x888888)
    } else {
      rgb(0x666666)
    };

    for (line_idx, line_number) in &prepaint.line_numbers {
      let y = bounds.top() + prepaint.line_height * (*line_idx - prepaint.viewport.start) as f32;

      let runs = vec![TextRun {
        len: line_number.len(),
        font: text_style.font(),
        color: text_color.into(),
        background_color: None,
        underline: None,
        strikethrough: None,
      }];

      let shaped =
        window
          .text_system()
          .shape_line(line_number.clone().into(), font_size, &runs, None);

      // Align to the right with padding
      let text_width = shaped.width;
      let right_padding = px(8.0);
      let x = bounds.right() - text_width - right_padding;

      let line_origin = point(x, y);
      shaped
        .paint(
          line_origin,
          prepaint.line_height,
          TextAlign::Right,
          None,
          window,
          cx,
        )
        .ok();
    }
  }
}
