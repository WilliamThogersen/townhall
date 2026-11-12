use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    px, App, Bounds, ElementId, FontStyle, FontWeight, GlobalElementId, HighlightStyle,
    InspectorElementId, IntoElement, LayoutId, Pixels, SharedString, StrikethroughStyle,
    StyledText, TextLayout, UnderlineStyle, Window,
};

use crate::markdown::Inline;
use crate::selection::{look_runs, BlockKey, Look};
use crate::theme::Theme;

pub struct BlockRef {
    pub layout: TextLayout,
    pub text: SharedString,
    pub links: Vec<(Range<usize>, String)>,
}

pub type Registry = Rc<RefCell<HashMap<BlockKey, BlockRef>>>;

pub fn highlight_style(look: &Look, theme: &Theme) -> HighlightStyle {
    let p = theme.palette;
    let mut style = HighlightStyle::default();
    if look.style.bold {
        style.font_weight = Some(FontWeight::BOLD);
    }
    if look.style.italic {
        style.font_style = Some(FontStyle::Italic);
    }
    if look.style.strike {
        style.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.0),
            color: Some(p.fg_faint.into()),
        });
        style.color = Some(p.fg_faint.into());
    }
    if look.style.code {
        style.background_color = Some(p.surface_2.into());
        style.color = Some(p.fg.into());
    }
    if look.link {
        style.color = Some(p.info.into());
        style.underline = Some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(p.info.into()),
            wavy: false,
        });
    }
    if look.selected {
        style.background_color = Some(p.selection.into());
    }
    style
}

pub struct SelectableText {
    text: StyledText,
    key: BlockKey,
    source: SharedString,
    links: Vec<(Range<usize>, String)>,
    registry: Registry,
}

impl SelectableText {
    pub fn new(
        key: BlockKey,
        inline: &Inline,
        selected: Option<Range<usize>>,
        theme: &Theme,
        registry: Registry,
    ) -> Self {
        let source: SharedString = inline.text.clone().into();
        let mut text = StyledText::new(source.clone());
        if !inline.text.is_empty() {
            let runs = look_runs(inline.text.len(), &inline.spans, selected);
            text = text.with_highlights(
                runs.into_iter()
                    .map(|(range, look)| (range, highlight_style(&look, theme))),
            );
        }
        let links = inline
            .spans
            .iter()
            .filter_map(|s| s.link.clone().map(|url| (s.range.clone(), url)))
            .collect();
        Self {
            text,
            key,
            source,
            links,
            registry,
        }
    }
}

impl gpui::Element for SelectableText {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.text.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.text
            .prepaint(id, inspector_id, bounds, state, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.text
            .paint(id, inspector_id, bounds, state, prepaint, window, cx);
        self.registry.borrow_mut().insert(
            self.key,
            BlockRef {
                layout: self.text.layout().clone(),
                text: self.source.clone(),
                links: self.links.clone(),
            },
        );
    }
}

impl IntoElement for SelectableText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::Style;
    use crate::theme::Mode;

    fn look(style: Style, link: bool, selected: bool) -> Look {
        Look {
            style,
            link,
            selected,
        }
    }

    #[test]
    fn a_plain_run_has_no_highlight() {
        let t = Theme::new(Mode::Dark);
        assert_eq!(
            highlight_style(&look(Style::default(), false, false), &t),
            HighlightStyle::default()
        );
    }

    #[test]
    fn bold_italic_code_strike_and_links_are_styled_apart() {
        let t = Theme::new(Mode::Dark);
        let p = t.palette;
        let bold = highlight_style(
            &look(
                Style {
                    bold: true,
                    ..Style::default()
                },
                false,
                false,
            ),
            &t,
        );
        assert_eq!(bold.font_weight, Some(FontWeight::BOLD));
        let italic = highlight_style(
            &look(
                Style {
                    italic: true,
                    ..Style::default()
                },
                false,
                false,
            ),
            &t,
        );
        assert_eq!(italic.font_style, Some(FontStyle::Italic));
        let code = highlight_style(
            &look(
                Style {
                    code: true,
                    ..Style::default()
                },
                false,
                false,
            ),
            &t,
        );
        assert_eq!(code.background_color, Some(p.surface_2.into()));
        let strike = highlight_style(
            &look(
                Style {
                    strike: true,
                    ..Style::default()
                },
                false,
                false,
            ),
            &t,
        );
        assert!(strike.strikethrough.is_some());
        assert_eq!(strike.color, Some(p.fg_faint.into()));
        let link = highlight_style(&look(Style::default(), true, false), &t);
        assert_eq!(link.color, Some(p.info.into()));
        assert!(link.underline.is_some());
    }

    #[test]
    fn selection_paints_the_selection_colour_over_the_code_background() {
        let t = Theme::new(Mode::Dark);
        let style = highlight_style(
            &look(
                Style {
                    code: true,
                    ..Style::default()
                },
                false,
                true,
            ),
            &t,
        );
        assert_eq!(style.background_color, Some(t.palette.selection.into()));
    }
}
