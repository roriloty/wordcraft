//! Touch selection on a tablet: a handle at each end of a text selection, dragged with a finger to
//! move that end, and the place the system's edit menu points at once a touch has made a
//! selection. The app turns it on where fingers are the pointer ([`WordApp::set_touch_selection`],
//! iPadOS). Its window layer reads the handles' areas and the menu's place
//! ([`WordApp::take_touch_overlay`]): a finger on a handle drags it at once instead of scrolling the
//! page, and the menu (Cut, Copy, Paste, Select All, the formatting toggles and the Font dialog)
//! sends its choices back as their keyboard shortcuts.

use craft::behavior::{Cursor, CursorIcon, HitLevel, HitPriority, Metrics, PartKey, PointerButton, PressPhase, Sense};
use craft::pages::ui::{PageBody, PlacedPage};
use craft::ui::contract::{Common, Component, Cx, MeasureCx, PaintUi, PartsCx, Pointer as _, Props, Role, Show as _};
use craft::ui::primitives::{PaintShape, StrokeWidth};
use craft::ui::style::{ComponentStyle, StyleColor, StyleCx};
use egui::{Pos2, Rect, Response, Ui, Vec2, pos2, vec2};
use serde_json::json;
use wordcraft_doc::Pos;
use wordcraft_engine::Selection;
use wordcraft_layout::DocLayout;

use crate::WordApp;
use crate::table_resize::{page_at, page_point, screen_of};

/// The part of the handle at the selection's start.
const START: PartKey = PartKey::new("selection_start_handle");
/// The part of the handle at the selection's end.
const END: PartKey = PartKey::new("selection_end_handle");
/// The radius of a handle's knob (points).
const KNOB: f32 = 6.0;
/// Half the width of a handle's touch area: 44 points across, the least a finger hits reliably.
const REACH: f32 = 22.0;
/// How far past the knob the touch area reaches (points).
const SLACK: f32 = 10.0;

/// The touch selection's state between passes.
#[derive(Default)]
pub struct TouchState {
    /// On where fingers are the pointer (the app's choice).
    pub(crate) enabled: bool,
    /// The latest pointer input was a finger (the window layer says so each frame).
    pub(crate) last_input_touch: bool,
    /// The handles' touch areas this pass (screen points).
    pub(crate) grab: Vec<Rect>,
    /// Where the edit menu should point, once, for a selection a touch just made (screen points).
    pub(crate) edit_menu: Option<Rect>,
    /// The selection the menu was last offered for.
    menu_for: Option<Selection>,
    /// A handle being dragged.
    drag: Option<HandleDrag>,
}

#[derive(Clone)]
struct HandleDrag {
    /// The end that stays where it is.
    fixed: Pos,
    /// From the finger to the line the handle stands on, so the finger doesn't cover the text.
    offset: Vec2,
}

/// What the window layer needs of the touch selection this frame.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TouchOverlay {
    /// The handles' touch areas (screen points): a finger here drags at once.
    pub grab: Vec<Rect>,
    /// Show the system's edit menu pointing at this part of the selection (screen points).
    pub edit_menu: Option<Rect>,
}

impl WordApp {
    /// Turns the touch selection on: handles on a selection made by touch, and the system edit
    /// menu offered for it (`take_touch_overlay`).
    pub fn set_touch_selection(&mut self, on: bool) {
        self.canvas.touch.enabled = on;
    }

    /// Whether the latest pointer input came from a finger, as the window layer knows it.
    pub fn set_touch_input(&mut self, touch: bool) {
        self.canvas.touch.last_input_touch = touch;
    }

    /// The handles' touch areas, and the edit menu to show once.
    pub fn take_touch_overlay(&mut self) -> TouchOverlay {
        TouchOverlay { grab: self.canvas.touch.grab.clone(), edit_menu: self.canvas.touch.edit_menu.take() }
    }
}

/// The handle's colour.
#[derive(Clone, Debug, PartialEq, ComponentStyle)]
pub(crate) struct TouchSelectionStyle {
    #[style(metrics)]
    metrics: Metrics,
    accent: StyleColor,
}

/// One handle of this pass: its part, where its line stands (screen) and which end it is.
struct Handle {
    key: PartKey,
    top: Pos2,
    bottom: Pos2,
    start: bool,
}

/// The selection's two handles over the page view.
#[derive(Component, Props)]
#[component(canvas_control)]
pub(crate) struct TouchSelection<'a> {
    #[common]
    common: Common,
    app: &'a mut WordApp,
    layout: &'a DocLayout,
    pages: &'a [PlacedPage],
    clip: Rect,
    handles: Vec<Handle>,
}

impl<'a> TouchSelection<'a> {
    /// The handles at the ends of `rects`, the selection's rectangles in document order (screen).
    fn new(app: &'a mut WordApp, layout: &'a DocLayout, pages: &'a [PlacedPage], clip: Rect, rects: &[Rect]) -> Self {
        let mut handles = Vec::new();
        if let (Some(first), Some(last)) = (rects.first(), rects.last()) {
            handles.push(Handle { key: START, top: first.left_top(), bottom: first.left_bottom(), start: true });
            handles.push(Handle { key: END, top: last.right_top(), bottom: last.right_bottom(), start: false });
        }
        handles.retain(|h| clip.contains(h.top) || clip.contains(h.bottom));
        TouchSelection { common: Common::new(), app, layout, pages, clip, handles }
    }

    /// Where a finger may take hold of `h`: around its line and its knob.
    fn area(h: &Handle) -> Rect {
        let (top, bottom) =
            if h.start { (h.top.y - 2.0 * KNOB - SLACK, h.bottom.y + SLACK) } else { (h.top.y - SLACK, h.bottom.y + 2.0 * KNOB + SLACK) };
        Rect::from_min_max(pos2(h.top.x - REACH, top), pos2(h.top.x + REACH, bottom))
    }

    /// The document position under the screen point `p`, in the selection's story.
    fn hit(&self, p: Pos2) -> Option<(usize, Pos)> {
        let placed = page_at(self.pages, p)?;
        let (x, y) = page_point(placed, p);
        let story = self.app.session.sel.focus.story;
        self.layout.hit(placed.page, x, y, story).map(|pos| (placed.page, pos))
    }

    fn press(&mut self, start: bool, origin: Pos2, line_middle: Pos2) {
        let (from, to) = self.app.session.sel.ordered();
        let fixed = if start { to } else { from };
        self.app.canvas.touch.drag = Some(HandleDrag { fixed, offset: line_middle - origin });
    }

    /// The finger at `pos`: the dragged end moves to the text under the handle's line.
    fn drag(&mut self, pos: Pos2) {
        let Some(d) = self.app.canvas.touch.drag.clone() else { return };
        let Some((page, at)) = self.hit(pos + d.offset) else { return };
        if at == d.fixed || at == self.app.session.sel_ends().focus {
            return;
        }
        let (Ok(fixed), Ok(at)) = (serde_json::to_value(&d.fixed), serde_json::to_value(&at)) else { return };
        // Straight to the session, as a drag-select does: the app's `run` would scroll on every frame.
        let _ = self.app.session.run("caret.set", &json!({"pos": fixed}));
        let _ = self.app.session.run("caret.set", &json!({"pos": at, "extend": true}));
        self.app.session.page_hint = page;
    }

    fn release(&mut self) {
        self.app.canvas.touch.drag = None;
        // The new selection gets the menu again.
        self.app.canvas.touch.menu_for = None;
    }
}

impl Component for TouchSelection<'_> {
    const ROLE: Role = Role::None;
    const OVERLAYS: bool = false;
    const STOPS: bool = false;
    const SHOWS_LABEL: bool = false;
    type Style = TouchSelectionStyle;
    type Output = Response;

    fn style(cx: &StyleCx<'_>, _: TouchSelectionStyleKey) -> TouchSelectionStyle {
        TouchSelectionStyle { metrics: Metrics::new(), accent: cx.color(cx.tokens.color.accent.solid) }
    }

    fn measure(&mut self, _: &MeasureCx<'_, Self>) -> Vec2 {
        Vec2::ZERO
    }

    fn parts(&mut self, _rect: Rect, cx: &mut PartsCx<'_, Self>) {
        let mut grab = Vec::new();
        for h in &self.handles {
            let area = Self::area(h).intersect(self.clip);
            if !area.is_positive() {
                continue;
            }
            cx.part(h.key, area)
                .sense(Sense::DRAG)
                .priority(HitPriority::new(HitLevel::Handle, 0))
                .cursor(Cursor::from(CursorIcon::Grab))
                .drag_cursor(CursorIcon::Grabbing)
                .pointer_only("Shift with the arrow keys moves the selection's end the same way");
            grab.push(area);
        }
        self.app.canvas.touch.grab = grab;
    }

    fn update(&mut self, cx: &mut Cx<'_, Self>) {
        let handles: Vec<(PartKey, bool, Pos2)> = self.handles.iter().map(|h| (h.key, h.start, h.top.lerp(h.bottom, 0.5))).collect();
        for (key, start, middle) in handles {
            let Some(result) = cx.part_if_declared(key) else { continue };
            match result.press.phase {
                PressPhase::Down { start: s, .. } if s.button == PointerButton::Primary => self.press(start, s.origin, middle),
                PressPhase::DragStart { pos, .. } | PressPhase::Drag { pos, .. } => self.drag(pos),
                PressPhase::Release { .. } | PressPhase::Cancel { .. } => self.release(),
                _ => {}
            }
        }
    }

    /// Each handle: a line along the selection's edge and a knob, above the start and below the end.
    fn paint(&mut self, ui: &mut PaintUi<'_>, cx: &Cx<'_, Self>) {
        let draw = ui.draw();
        let accent = cx.style.accent;
        for h in &self.handles {
            draw.stroke(&PaintShape::Path(vec![h.top, h.bottom]), StrokeWidth::Medium, accent);
            let knob = if h.start { h.top - vec2(0.0, KNOB) } else { h.bottom + vec2(0.0, KNOB) };
            draw.fill(&PaintShape::Circle(knob, KNOB), accent);
        }
    }

    fn finish(self, cx: Cx<'_, Self>) -> Response {
        cx.response.clone()
    }
}

/// The selection's rectangles (page points, document order) on screen.
fn screen_rects(selected: &[(usize, wordcraft_geom::Rect)], pages: &[PlacedPage]) -> Vec<Rect> {
    selected
        .iter()
        .filter_map(|(page, r)| {
            let placed = pages.iter().find(|p| p.page == *page)?;
            Some(Rect::from_two_pos(screen_of(placed, r.x, r.y), screen_of(placed, r.x + r.w, r.y + r.h)))
        })
        .collect()
}

/// Shows the handles over the page view's body, and settles the edit menu for a selection a touch
/// just finished. `selected` is this pass's selection, in page points.
pub(crate) fn show(app: &mut WordApp, ui: &mut Ui, layout: &DocLayout, body: &PageBody, selected: &[(usize, wordcraft_geom::Rect)]) {
    let touch = &app.canvas.touch;
    let dragging = touch.drag.is_some();
    let wanted = touch.enabled
        && (touch.last_input_touch || dragging)
        && !app.session.sel.is_collapsed()
        && !app.session.view.read_mode
        && !app.modal_open()
        && !app.ui.backstage;
    if !wanted {
        app.canvas.touch.grab.clear();
        app.canvas.touch.drag = None;
        if app.session.sel.is_collapsed() {
            app.canvas.touch.menu_for = None;
        }
        return;
    }
    let rects = screen_rects(selected, body.pages);
    TouchSelection::new(app, layout, body.pages, body.view.rect, &rects).show_at(ui, body.rect);
    // The edit menu, once per selection, when the finger that made it is up.
    let busy = app.canvas.drag_selecting() || app.canvas.touch.drag.is_some() || ui.input(|i| i.pointer.any_down());
    if !busy && app.canvas.touch.menu_for.as_ref() != Some(&app.session.sel) {
        let shown = rects.iter().filter_map(|r| Some(r.intersect(body.view.rect)).filter(|v| v.is_positive())).reduce(|a, b| a.union(b));
        if let Some(rect) = shown {
            app.canvas.touch.edit_menu = Some(rect);
            app.canvas.touch.menu_for = Some(app.session.sel.clone());
        }
    }
}
