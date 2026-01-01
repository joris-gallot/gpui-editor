use gpui::{
  App, Bounds, DispatchPhase, ElementId, ElementInputHandler, Entity, GlobalElementId,
  InspectorElementId, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
  PaintQuad, Pixels, Point, ScrollDelta, ScrollWheelEvent, ShapedLine, Style, TextRun, Window,
  blue, fill, point, prelude::*, px, relative, rgba, size,
};
use std::{ops::Range, rc::Rc, sync::Arc};

use crate::{document::Document, editor::Editor};

/// Encapsulates layout information for mouse position -> text offset conversion
#[derive(Clone)]
pub struct PositionMap {
  pub shaped_lines: Vec<(usize, Arc<ShapedLine>)>,
  pub bounds: Bounds<Pixels>,
  pub line_height: Pixels,
  pub viewport: Range<usize>,
}

impl PositionMap {
  pub fn point_for_position(&self, position: Point<Pixels>, document: &Document) -> Option<usize> {
    if !self.bounds.contains(&position) {
      return None;
    }

    if document.is_empty() {
      return Some(0);
    }

    let y_offset = position.y - self.bounds.top();
    let row_in_viewport = (y_offset / self.line_height).floor() as usize;
    let actual_row = self.viewport.start + row_in_viewport;

    if actual_row >= document.len_lines() {
      return Some(document.len());
    }

    let shaped = self
      .shaped_lines
      .iter()
      .find(|(idx, _)| *idx == actual_row)
      .map(|(_, s)| s)?;

    let x_offset = position.x - self.bounds.left();
    let column = shaped.closest_index_for_x(x_offset);

    let line_start = document.line_to_char(actual_row);
    Some(line_start + column)
  }
}

pub struct EditorElement {
  editor: Entity<Editor>,
}

pub struct PrepaintState {
  shaped_lines: Vec<(usize, Arc<ShapedLine>)>,
  cursor_quad: Option<PaintQuad>,
  selection_quads: Vec<PaintQuad>,
  viewport: Range<usize>,
  bounds: Bounds<Pixels>,
  line_height: Pixels,
}

impl EditorElement {
  pub fn new(editor: Entity<Editor>) -> Self {
    Self { editor }
  }

  fn calculate_viewport(
    &self,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    scroll_offset: f32,
    total_lines: usize,
  ) -> Range<usize> {
    let visible_line_count = ((bounds.size.height / line_height).ceil() as usize).max(1);

    let start_line = (scroll_offset.floor() as usize).min(total_lines.saturating_sub(1));
    let end_line = (start_line + visible_line_count).min(total_lines);

    start_line..end_line
  }
}

impl IntoElement for EditorElement {
  type Element = Self;

  fn into_element(self) -> Self::Element {
    self
  }
}

impl Element for EditorElement {
  type RequestLayoutState = ();
  type PrepaintState = PrepaintState;

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
    let (viewport, selected_range, cursor_offset, mut shaped_lines, lines_to_shape) = {
      let editor = self.editor.read(cx);
      let document = editor.document().read(cx);
      let line_height = window.line_height();
      let scroll_offset = editor.scroll_offset;

      let viewport =
        self.calculate_viewport(bounds, line_height, scroll_offset, document.len_lines());

      let mut lines_to_shape = Vec::new();
      let mut shaped_lines = Vec::new();

      for line_idx in viewport.clone() {
        if line_idx >= document.len_lines() {
          break;
        }
        match editor.line_layouts.get(&line_idx) {
          Some(shaped) => {
            // Arc::clone is cheap - just incrementing reference count
            shaped_lines.push((line_idx, Arc::clone(shaped)));
          }
          None => {
            let line_content = document.line_content(line_idx).unwrap_or_default();
            lines_to_shape.push((line_idx, line_content));
          }
        }
      }

      (
        viewport,
        editor.selected_range.clone(),
        editor.cursor_offset(),
        shaped_lines,
        lines_to_shape,
      )
    };

    let style = window.text_style();
    let font_size = style.font_size.to_pixels(window.rem_size());
    let line_height = window.line_height();

    let mut newly_shaped = Vec::new();
    for (line_idx, line_content) in lines_to_shape {
      let run = TextRun {
        len: line_content.len(),
        font: style.font(),
        color: style.color,
        background_color: None,
        underline: None,
        strikethrough: None,
      };
      let shaped = window
        .text_system()
        .shape_line(line_content.into(), font_size, &[run], None);
      newly_shaped.push((line_idx, shaped));
    }

    if !newly_shaped.is_empty() {
      self.editor.update(cx, |editor, _| {
        for (line_idx, shaped) in newly_shaped {
          // Wrap in Arc for cheap cloning
          let shaped_arc = Arc::new(shaped);
          editor.line_layouts.insert(line_idx, shaped_arc.clone());
          shaped_lines.push((line_idx, shaped_arc));
        }
        // Limit cache size to prevent memory issues with large files
        editor.ensure_cache_size(viewport.clone());
      });
    }

    let document = self.editor.read(cx).document().read(cx);

    let cursor_line = document.char_to_line(cursor_offset);
    let cursor_quad = if viewport.contains(&cursor_line) {
      let shaped_opt = shaped_lines
        .iter()
        .find(|(idx, _)| *idx == cursor_line)
        .map(|(_, shaped)| shaped);
      if let Some(shaped) = shaped_opt {
        let line_start = document.line_to_char(cursor_line);
        let cursor_in_line = cursor_offset - line_start;
        let cursor_x = shaped.x_for_index(cursor_in_line);
        let y = bounds.top() + line_height * (cursor_line - viewport.start) as f32;
        Some(fill(
          Bounds::new(
            point(bounds.left() + cursor_x, y),
            size(px(2.), line_height),
          ),
          blue(),
        ))
      } else {
        None
      }
    } else {
      None
    };

    let mut selection_quads = Vec::new();
    if !selected_range.is_empty() {
      let sel_start = selected_range.start;
      let sel_end = selected_range.end;
      let sel_start_line = document.char_to_line(sel_start);
      let sel_end_line = document.char_to_line(sel_end);

      for line_idx in sel_start_line..=sel_end_line {
        if !viewport.contains(&line_idx) {
          continue;
        }
        let line_range = document.line_range(line_idx).unwrap();
        let shaped_opt = shaped_lines
          .iter()
          .find(|(idx, _)| *idx == line_idx)
          .map(|(_, shaped)| shaped);

        if let Some(shaped) = shaped_opt {
          let line_start = line_range.start;
          let line_end = line_range.end;
          let sel_line_start = sel_start.max(line_start) - line_start;
          let sel_line_end = sel_end.min(line_end) - line_start;
          let x_start = shaped.x_for_index(sel_line_start);
          let x_end = shaped.x_for_index(sel_line_end);
          let y = bounds.top() + line_height * (line_idx - viewport.start) as f32;

          // TODO: Improve visual for newline selection
          // If selection is empty on this line (selecting just the newline),
          let visual_x_end = if x_start == x_end {
            x_end + px(4.0) // Small width to show newline selection
          } else {
            x_end
          };

          selection_quads.push(fill(
            Bounds::from_corners(
              point(bounds.left() + x_start, y),
              point(bounds.left() + visual_x_end, y + line_height),
            ),
            rgba(0x3311ff30),
          ));
        }
      }
    }

    PrepaintState {
      shaped_lines,
      cursor_quad,
      selection_quads,
      viewport,
      bounds,
      line_height,
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
    let (focus_handle, is_focused) = {
      let editor = self.editor.read(cx);
      (
        editor.focus_handle.clone(),
        editor.focus_handle.is_focused(window),
      )
    };

    window.handle_input(
      &focus_handle,
      ElementInputHandler::new(bounds, self.editor.clone()),
      cx,
    );

    // Use Rc to avoid cloning PositionMap in closures
    let position_map = Rc::new(PositionMap {
      shaped_lines: prepaint.shaped_lines.clone(),
      bounds: prepaint.bounds,
      line_height: prepaint.line_height,
      viewport: prepaint.viewport.clone(),
    });

    window.on_mouse_event({
      let editor = self.editor.clone();
      let position_map = Rc::clone(&position_map);
      move |event: &MouseDownEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
          editor.update(cx, |editor, cx| {
            editor.mouse_left_down(event, &position_map, window, cx);
          });
        }
      }
    });

    window.on_mouse_event({
      let editor = self.editor.clone();
      move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
          editor.update(cx, |editor, cx| {
            editor.mouse_left_up(event, window, cx);
          });
        }
      }
    });

    window.on_mouse_event({
      let editor = self.editor.clone();
      let position_map = Rc::clone(&position_map);
      move |event: &MouseMoveEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble {
          let is_selecting = editor.read(cx).is_selecting;
          if is_selecting {
            editor.update(cx, |editor, cx| {
              editor.mouse_dragged(event, &position_map, window, cx);
            });
          }
        }
      }
    });

    // Handle mouse wheel scroll
    window.on_mouse_event({
      let editor = self.editor.clone();
      move |event: &ScrollWheelEvent, phase, _window, cx| {
        if phase == DispatchPhase::Bubble {
          editor.update(cx, |editor, cx| {
            let document = editor.document().read(cx);
            let total_lines = document.len_lines();

            // Extract scroll delta (handle both pixel and line scrolling)
            // Note: Negative delta because scrolling down should increase scroll_offset
            let scroll_delta = match event.delta {
              ScrollDelta::Pixels(point) => -(point.y / px(20.0)), // Pixel scrolling (trackpad)
              ScrollDelta::Lines(point) => -(point.y * 3.0),       // Line scrolling (mouse wheel)
            };

            let new_scroll = (editor.scroll_offset + scroll_delta)
              .max(0.0)
              .min((total_lines.saturating_sub(1)) as f32);

            editor.scroll_offset = new_scroll;
            cx.notify();
          });
        }
      }
    });

    // Paint selection
    for quad in &prepaint.selection_quads {
      window.paint_quad(quad.clone());
    }

    // Paint text lines
    for (line_idx, shaped_line) in &prepaint.shaped_lines {
      let y = bounds.top() + prepaint.line_height * (*line_idx - prepaint.viewport.start) as f32;
      shaped_line
        .paint(point(bounds.left(), y), prepaint.line_height, window, cx)
        .ok();
    }

    // Paint cursor (if focused)
    if is_focused && let Some(cursor_quad) = &prepaint.cursor_quad {
      window.paint_quad(cursor_quad.clone());
    }
  }
}
