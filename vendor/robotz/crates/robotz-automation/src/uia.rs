use anyhow::Result;
use async_trait::async_trait;
/// Windows UI Automation tool (Windows only)
/// Supports 25+ actions for element interaction, keyboard control, and window management.
use robotz_core::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};

pub struct UiaTool;

#[async_trait]
impl Tool for UiaTool {
    fn name(&self) -> &str {
        "uia"
    }

    fn description(&self) -> &str {
        "Control Windows desktop applications via UI Automation (UIA). \
         Supports finding controls, clicking, typing, keyboard shortcuts, scrolling, drag-drop, window management. \
         \
         Recommended workflow: \
         1. list_windows — find the target window title \
         2. find (with window_title) — locate a specific control by name/type \
         3. click / type / send_hotkey — interact with the control \
         \
         When find fails (element not found): \
         → Use screen_capture to take a screenshot and visually locate the element \
         → Then use click with x/y coordinates instead of name-based find \
         → Or use smart_find which tries fuzzy matching \
         \
         Tips: \
         - Always specify window_title to narrow the search scope \
         - Use annotate_elements to get a labeled map of all controls in a window \
         - For text input fields, use 'type' action (not 'click' then 'type') \
         - send_hotkey supports: ctrl+c, ctrl+v, alt+f4, win+d, ctrl+shift+esc, etc. It requires window_title \
           and the target window must already be in the foreground (call activate_window first)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "list_windows", "find", "get_children", "get_rect", "get_value",
                        "click", "double_click", "right_click", "hover",
                        "type", "send_hotkey", "send_keys",
                        "get_text", "scroll", "drag_drop",
                        "expand", "collapse", "select", "check", "uncheck",
                        "wait_for_element",
                        "activate_window", "minimize", "maximize", "restore",
                        "close_window", "move_window", "resize_window", "get_window_rect",
                        "smart_find", "annotate_elements"
                    ],
                    "description": "Action to perform"
                },
                "name": {
                    "type": "string",
                    "description": "Control name to search for"
                },
                "class_name": {
                    "type": "string",
                    "description": "Control class name"
                },
                "automation_id": {
                    "type": "string",
                    "description": "Control automation ID"
                },
                "control_type": {
                    "type": "string",
                    "description": "Control type filter (e.g. Button, Edit, ListItem, CheckBox, ComboBox, TreeItem)"
                },
                "window_title": {
                    "type": "string",
                    "description": "Limit search to children of a specific window by title"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type (for 'type' action) or option to select (for 'select' action)"
                },
                "hotkey": {
                    "type": "string",
                    "description": "Hotkey combination for 'send_hotkey' (e.g. 'ctrl+c', 'alt+f4', 'win+d')"
                },
                "keys": {
                    "type": "string",
                    "description": "Key sequence for 'send_keys' (e.g. '{Enter}', '{Tab}', '{Escape}')"
                },
                "x": {
                    "type": "integer",
                    "description": "X coordinate (for click by coords, drag start, or window move)"
                },
                "y": {
                    "type": "integer",
                    "description": "Y coordinate (for click by coords, drag start, or window move)"
                },
                "x2": {
                    "type": "integer",
                    "description": "Target X coordinate (for drag_drop end, or window resize width)"
                },
                "y2": {
                    "type": "integer",
                    "description": "Target Y coordinate (for drag_drop end, or window resize height)"
                },
                "direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                    "description": "Scroll direction"
                },
                "amount": {
                    "type": "integer",
                    "description": "Scroll amount (number of ticks, default 3)"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Timeout in milliseconds for wait_for_element (default 10000)"
                },
                "depth": {
                    "type": "integer",
                    "description": "Depth for get_children traversal (default 2, max 5)"
                },
                "description": {
                    "type": "string",
                    "description": "Natural language description of the element to find (for smart_find)"
                },
                "max_elements": {
                    "type": "integer",
                    "description": "Maximum number of elements to annotate (for annotate_elements, default 30)"
                },
                "logical_coords": {
                    "type": "boolean",
                    "description": "Deprecated. Coordinates from screen_capture are already physical pixels because the process is declared Per-Monitor DPI aware in the app manifest. This flag is kept for backwards compatibility only — it is a no-op."
                },
                "_skip_calibration": {
                    "type": "boolean",
                    "description": "INTERNAL — set to true only by the manual calibration Phase 2 runner so the cursor reaches the raw physical target without re-applying the (still-being-fit) calibration on top of itself. Do not use from agent prompts."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let action = match input["action"].as_str() {
            Some(a) => a,
            None => return Ok(ToolResult::err("Missing required parameter: action")),
        };

        let result = match action {
            // Discovery
            "list_windows" => self.list_windows(),
            "find" => self.find_element(&input),
            "get_children" => self.get_children(&input),
            "get_rect" => self.get_rect(&input),
            "get_value" => self.get_value(&input),
            "get_text" => self.get_text(&input),
            // Mouse actions
            "click" => self.click_element(&input),
            "double_click" => self.double_click_element(&input),
            "right_click" => self.right_click_element(&input),
            "hover" => self.hover_element(&input),
            "scroll" => self.scroll_element(&input),
            "drag_drop" => self.drag_drop(&input),
            // Keyboard actions
            "type" => self.type_text(&input),
            "send_hotkey" => self.send_hotkey(&input),
            "send_keys" => self.send_keys_action(&input),
            // State actions
            "expand" => self.expand_element(&input),
            "collapse" => self.collapse_element(&input),
            "select" => self.select_item(&input),
            "check" => self.set_check(&input, true),
            "uncheck" => self.set_check(&input, false),
            // Wait
            "wait_for_element" => self.wait_for_element(&input, ctx).await,
            // Window management
            "activate_window" => self.activate_window(&input),
            "minimize" => self.window_state(&input, "minimize"),
            "maximize" => self.window_state(&input, "maximize"),
            "restore" => self.window_state(&input, "restore"),
            "close_window" => self.close_window(&input),
            "move_window" => self.move_window(&input),
            "resize_window" => self.resize_window(&input),
            "get_window_rect" => self.get_window_rect(&input),
            // Hybrid vision automation
            "smart_find" => self.smart_find(&input, ctx).await,
            "annotate_elements" => self.annotate_elements(&input, ctx).await,
            _ => Ok(ToolResult::err(format!("Unknown action: {}", action))),
        };
        // Only successful injections advance the agent-input baseline. A refused action must not
        // mark the time, or it would mask the user input that caused the refusal.
        if input_guard::INJECTING_ACTIONS.contains(&action)
            && matches!(&result, Ok(r) if !r.is_error)
        {
            input_guard::mark_agent_input();
        }
        result
    }
}

// ─── Helper: build a matcher from common search params ───────────────────────

impl UiaTool {
    fn build_matcher(
        &self,
        automation: &uiautomation::UIAutomation,
        root: uiautomation::UIElement,
        input: &Value,
        timeout_ms: u64,
    ) -> uiautomation::UIMatcher {
        let mut matcher = automation.create_matcher().from(root).timeout(timeout_ms);
        if let Some(name) = input["name"].as_str() {
            matcher = matcher.name(name);
        }
        // automation_id is a unique control identifier (e.g. "btnOK"), distinct from name (display text)
        // UIMatcher has no built-in automation_id filter, so use filter_fn
        if let Some(aid) = input["automation_id"].as_str().map(|s| s.to_string()) {
            matcher = matcher.filter_fn(Box::new(move |el: &uiautomation::UIElement| {
                Ok(el.get_automation_id().unwrap_or_default() == aid)
            }));
        }
        if let Some(class) = input["class_name"].as_str() {
            matcher = matcher.classname(class);
        }
        // control_type filter: map common names to Win32 classnames
        if let Some(ct) = input["control_type"].as_str() {
            let classname = match ct.to_lowercase().as_str() {
                "button" => "Button",
                "edit" | "textbox" => "Edit",
                "combobox" => "ComboBox",
                "listbox" => "ListBox",
                "listview" => "SysListView32",
                "treeview" => "SysTreeView32",
                "toolbar" => "ToolbarWindow32",
                "statusbar" => "msctls_statusbar32",
                "tabcontrol" => "SysTabControl32",
                _ => ct,
            };
            matcher = matcher.classname(classname);
        }
        matcher
    }

    /// Get root element, optionally scoped to a window by title
    fn get_search_root(
        &self,
        automation: &uiautomation::UIAutomation,
        input: &Value,
    ) -> Result<uiautomation::UIElement> {
        let root = automation
            .get_root_element()
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        if let Some(title) = input["window_title"].as_str() {
            let walker = automation
                .get_control_view_walker()
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if let Ok(child) = walker.get_first_child(&root) {
                let mut current = child;
                loop {
                    let name = current.get_name().unwrap_or_default();
                    if name.contains(title) {
                        return Ok(current);
                    }
                    match walker.get_next_sibling(&current) {
                        Ok(next) => current = next,
                        Err(_) => break,
                    }
                }
            }
            return Err(anyhow::anyhow!(
                "Window with title containing '{}' not found",
                title
            ));
        }
        Ok(root)
    }

    // ─── Discovery ───────────────────────────────────────────────────────────

    fn list_windows(&self) -> Result<ToolResult> {
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = automation
            .get_root_element()
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let walker = automation
            .get_control_view_walker()
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let mut windows = Vec::new();
        if let Ok(child) = walker.get_first_child(&root) {
            let mut current = child;
            loop {
                let name = current.get_name().unwrap_or_default();
                let class = current.get_classname().unwrap_or_default();
                if !name.is_empty() {
                    windows.push(format!("Name: '{}', Class: '{}'", name, class));
                }
                match walker.get_next_sibling(&current) {
                    Ok(next) => current = next,
                    Err(_) => break,
                }
            }
        }

        Ok(ToolResult::ok(format!(
            "Found {} windows:\n{}",
            windows.len(),
            windows.join("\n")
        )))
    }

    fn find_element(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);

        match matcher.find_first() {
            Ok(element) => {
                let name = element.get_name().unwrap_or_default();
                let class = element.get_classname().unwrap_or_default();
                Ok(ToolResult::ok(format!(
                    "Found element: Name='{}', Class='{}'",
                    name, class
                )))
            }
            Err(e) => Ok(ToolResult::err(format!("Element not found: {}", e))),
        }
    }

    fn get_children(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let depth = input["depth"].as_u64().unwrap_or(2).min(5) as usize;

        let start = if input["name"].is_null()
            && input["class_name"].is_null()
            && input["automation_id"].is_null()
        {
            root
        } else {
            let matcher = self.build_matcher(&automation, root, input, 5000);
            match matcher.find_first() {
                Ok(el) => el,
                Err(e) => return Ok(ToolResult::err(format!("Parent element not found: {}", e))),
            }
        };

        let walker = automation
            .get_control_view_walker()
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let mut results = Vec::new();
        self.collect_children(&walker, &start, 0, depth, &mut results);

        Ok(ToolResult::ok(format!(
            "Children ({} elements):\n{}",
            results.len(),
            results.join("\n")
        )))
    }

    fn collect_children(
        &self,
        walker: &uiautomation::UITreeWalker,
        element: &uiautomation::UIElement,
        current_depth: usize,
        max_depth: usize,
        results: &mut Vec<String>,
    ) {
        if current_depth >= max_depth {
            return;
        }
        if let Ok(child) = walker.get_first_child(element) {
            let mut current = child;
            loop {
                let name = current.get_name().unwrap_or_default();
                let class = current.get_classname().unwrap_or_default();
                let indent = "  ".repeat(current_depth);
                results.push(format!("{}Name='{}', Class='{}'", indent, name, class));
                self.collect_children(walker, &current, current_depth + 1, max_depth, results);
                match walker.get_next_sibling(&current) {
                    Ok(next) => current = next,
                    Err(_) => break,
                }
            }
        }
    }

    fn get_rect(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);

        match matcher.find_first() {
            Ok(element) => {
                let rect = element
                    .get_bounding_rectangle()
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                let left = rect.get_left();
                let top = rect.get_top();
                let right = rect.get_right();
                let bottom = rect.get_bottom();
                Ok(ToolResult::ok(format!(
                    "Rect: left={}, top={}, right={}, bottom={}, width={}, height={}",
                    left,
                    top,
                    right,
                    bottom,
                    right - left,
                    bottom - top
                )))
            }
            Err(e) => Ok(ToolResult::err(format!("Element not found: {}", e))),
        }
    }

    fn get_value(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::patterns::{UITextPattern, UIValuePattern};
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);

        match matcher.find_first() {
            Ok(element) => {
                let name = element.get_name().unwrap_or_default();

                // Try ValuePattern first (standard edit controls, combo boxes)
                if let Ok(pattern) = element.get_pattern::<UIValuePattern>() {
                    if let Ok(val) = pattern.get_value() {
                        if !val.is_empty() {
                            return Ok(ToolResult::ok(format!(
                                "Element: '{}'\nValue (ValuePattern): {}",
                                name, val
                            )));
                        }
                    }
                }

                // Try TextPattern (RichEdit, document controls)
                if let Ok(pattern) = element.get_pattern::<UITextPattern>() {
                    if let Ok(doc_range) = pattern.get_document_range() {
                        if let Ok(text) = doc_range.get_text(1_000_000) {
                            if !text.is_empty() {
                                return Ok(ToolResult::ok(format!(
                                    "Element: '{}'\nValue (TextPattern): {}",
                                    name, text
                                )));
                            }
                        }
                    }
                }

                // Win32 fallback: WM_GETTEXT for classic Edit / RichEdit controls
                if let Ok(handle) = element.get_native_window_handle() {
                    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
                    use windows::Win32::UI::WindowsAndMessaging::{
                        SendMessageW, WM_GETTEXT, WM_GETTEXTLENGTH,
                    };
                    let hwnd_isize: isize = handle.into();
                    let hwnd = HWND(hwnd_isize as *mut core::ffi::c_void);
                    let len = unsafe { SendMessageW(hwnd, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)) };
                    if len.0 > 0 {
                        let buf_len = (len.0 as usize) + 1;
                        let mut buf: Vec<u16> = vec![0u16; buf_len];
                        let copied = unsafe {
                            SendMessageW(
                                hwnd,
                                WM_GETTEXT,
                                WPARAM(buf_len),
                                LPARAM(buf.as_mut_ptr() as isize),
                            )
                        };
                        if copied.0 > 0 {
                            let text = String::from_utf16_lossy(&buf[..copied.0 as usize]);
                            if !text.is_empty() {
                                return Ok(ToolResult::ok(format!(
                                    "Element: '{}'\nValue (WM_GETTEXT): {}",
                                    name, text
                                )));
                            }
                        }
                    }
                }

                // Final fallback: element name only
                Ok(ToolResult::ok(format!(
                    "Element: '{}'\nNote: ValuePattern not supported by this control. Name returned as fallback.",
                    name
                )))
            }
            Err(e) => Ok(ToolResult::err(format!("Element not found: {}", e))),
        }
    }

    fn get_text(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::patterns::{UITextPattern, UIValuePattern};
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);

        match matcher.find_first() {
            Ok(element) => {
                let name = element.get_name().unwrap_or_default();

                // Try TextPattern first (rich text, document controls).
                // Use a large positive max_length instead of -1 to avoid crate-specific quirks
                // where -1 may be interpreted as "0 characters" on some controls.
                if let Ok(pattern) = element.get_pattern::<UITextPattern>() {
                    if let Ok(doc_range) = pattern.get_document_range() {
                        if let Ok(text) = doc_range.get_text(1_000_000) {
                            if !text.is_empty() {
                                return Ok(ToolResult::ok(format!(
                                    "Element: '{}'\nText (TextPattern): {}",
                                    name, text
                                )));
                            }
                        }
                    }
                }

                // Try ValuePattern as fallback (standard edit controls, combo boxes)
                if let Ok(pattern) = element.get_pattern::<UIValuePattern>() {
                    if let Ok(val) = pattern.get_value() {
                        if !val.is_empty() {
                            return Ok(ToolResult::ok(format!(
                                "Element: '{}'\nText (ValuePattern): {}",
                                name, val
                            )));
                        }
                    }
                }

                // Win32 fallback: use WM_GETTEXT for Edit / RichEdit controls that don't
                // expose TextPattern or ValuePattern via UIA (e.g. classic Notepad RichEdit).
                if let Ok(handle) = element.get_native_window_handle() {
                    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
                    use windows::Win32::UI::WindowsAndMessaging::{
                        SendMessageW, WM_GETTEXT, WM_GETTEXTLENGTH,
                    };
                    let hwnd_isize: isize = handle.into();
                    let hwnd = HWND(hwnd_isize as *mut core::ffi::c_void);
                    let len = unsafe { SendMessageW(hwnd, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)) };
                    if len.0 > 0 {
                        let buf_len = (len.0 as usize) + 1;
                        let mut buf: Vec<u16> = vec![0u16; buf_len];
                        let copied = unsafe {
                            SendMessageW(
                                hwnd,
                                WM_GETTEXT,
                                WPARAM(buf_len),
                                LPARAM(buf.as_mut_ptr() as isize),
                            )
                        };
                        if copied.0 > 0 {
                            let text = String::from_utf16_lossy(&buf[..copied.0 as usize]);
                            if !text.is_empty() {
                                return Ok(ToolResult::ok(format!(
                                    "Element: '{}'\nText (WM_GETTEXT): {}",
                                    name, text
                                )));
                            }
                        }
                    }
                }

                // Final fallback: element name only
                Ok(ToolResult::ok(format!(
                    "Element: '{}'\nNote: Could not read text content (TextPattern/ValuePattern/WM_GETTEXT all returned empty).",
                    name
                )))
            }
            Err(e) => Ok(ToolResult::err(format!("Element not found: {}", e))),
        }
    }

    // ─── Mouse Actions ────────────────────────────────────────────────────────

    fn click_element(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::inputs::Mouse;
        use uiautomation::types::Point;
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;

        if let (Some(x), Some(y)) = (input["x"].as_i64(), input["y"].as_i64()) {
            // Coordinates are physical screen pixels (same as screen_capture grid labels).
            // Apply any active manual calibration to compensate for residual drift
            // (e.g. inside VMs, RDP, mixed-DPI multi-monitor). When no calibration
            // is in effect this is a no-op.
            let (raw_x, raw_y) = (x as i32, y as i32);
            let bypass = input["_skip_calibration"].as_bool().unwrap_or(false);
            let (px, py) = if bypass {
                (raw_x, raw_y)
            } else {
                crate::calibration::apply_active_calibration(raw_x, raw_y)
            };
            if (px, py) != (raw_x, raw_y) {
                tracing::debug!(
                    "uia.click calibration: ({},{}) -> ({},{})",
                    raw_x,
                    raw_y,
                    px,
                    py
                );
            }
            Mouse::new()
                .click(&Point::new(px, py))
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            return Ok(ToolResult::ok(format!(
                "Clicked at ({}, {}){}",
                px,
                py,
                if (px, py) != (raw_x, raw_y) {
                    format!(" [calibrated from ({},{})]", raw_x, raw_y)
                } else {
                    String::new()
                }
            )));
        }

        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);
        match matcher.find_first() {
            Ok(element) => {
                // First try native invoke click; if that fails, retry with center-point mouse click.
                match element.click() {
                    Ok(_) => Ok(ToolResult::ok(format!(
                        "Clicked: '{}'",
                        element.get_name().unwrap_or_default()
                    ))),
                    Err(_) => {
                        let rect = element
                            .get_bounding_rectangle()
                            .map_err(|e| anyhow::anyhow!("{}", e))?;
                        let cx = (rect.get_left() + rect.get_right()) / 2;
                        let cy = (rect.get_top() + rect.get_bottom()) / 2;
                        Mouse::new()
                            .click(&Point::new(cx, cy))
                            .map_err(|e| anyhow::anyhow!("{}", e))?;
                        Ok(ToolResult::ok(format!(
                            "Clicked with coordinate fallback: '{}' at ({}, {})",
                            element.get_name().unwrap_or_default(),
                            cx,
                            cy
                        )))
                    }
                }
            }
            Err(e) => Ok(ToolResult::err(format!(
                "Element not found for click: {}",
                e
            ))),
        }
    }

    fn double_click_element(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::inputs::Mouse;
        use uiautomation::types::Point;
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;

        if let (Some(x), Some(y)) = (input["x"].as_i64(), input["y"].as_i64()) {
            // Physical screen pixel coordinates (same as screen_capture grid labels).
            let (raw_x, raw_y) = (x as i32, y as i32);
            let (px, py) = if input["_skip_calibration"].as_bool().unwrap_or(false) {
                (raw_x, raw_y)
            } else {
                crate::calibration::apply_active_calibration(raw_x, raw_y)
            };
            Mouse::new()
                .double_click(&Point::new(px, py))
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            return Ok(ToolResult::ok(format!(
                "Double-clicked at ({}, {})",
                px, py
            )));
        }

        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);
        match matcher.find_first() {
            Ok(element) => {
                let rect = element
                    .get_bounding_rectangle()
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                let cx = (rect.get_left() + rect.get_right()) / 2;
                let cy = (rect.get_top() + rect.get_bottom()) / 2;
                Mouse::new()
                    .double_click(&Point::new(cx, cy))
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                Ok(ToolResult::ok(format!(
                    "Double-clicked: '{}'",
                    element.get_name().unwrap_or_default()
                )))
            }
            Err(e) => Ok(ToolResult::err(format!(
                "Element not found for double_click: {}",
                e
            ))),
        }
    }

    fn right_click_element(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::inputs::Mouse;
        use uiautomation::types::Point;
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;

        if let (Some(x), Some(y)) = (input["x"].as_i64(), input["y"].as_i64()) {
            // Physical screen pixel coordinates (same as screen_capture grid labels).
            let (raw_x, raw_y) = (x as i32, y as i32);
            let (px, py) = if input["_skip_calibration"].as_bool().unwrap_or(false) {
                (raw_x, raw_y)
            } else {
                crate::calibration::apply_active_calibration(raw_x, raw_y)
            };
            Mouse::new()
                .right_click(&Point::new(px, py))
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            return Ok(ToolResult::ok(format!("Right-clicked at ({}, {})", px, py)));
        }

        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);
        match matcher.find_first() {
            Ok(element) => {
                let rect = element
                    .get_bounding_rectangle()
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                let cx = (rect.get_left() + rect.get_right()) / 2;
                let cy = (rect.get_top() + rect.get_bottom()) / 2;
                Mouse::new()
                    .right_click(&Point::new(cx, cy))
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                Ok(ToolResult::ok(format!(
                    "Right-clicked: '{}'",
                    element.get_name().unwrap_or_default()
                )))
            }
            Err(e) => Ok(ToolResult::err(format!(
                "Element not found for right_click: {}",
                e
            ))),
        }
    }

    fn hover_element(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::UIAutomation;

        if let (Some(x), Some(y)) = (input["x"].as_i64(), input["y"].as_i64()) {
            // Physical screen pixel coordinates. Use SendInput+VIRTUALDESK for multi-monitor support.
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
                MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT,
            };
            let (raw_x, raw_y) = (x as i32, y as i32);
            let (cx, cy) = if input["_skip_calibration"].as_bool().unwrap_or(false) {
                (raw_x, raw_y)
            } else {
                crate::calibration::apply_active_calibration(raw_x, raw_y)
            };
            let (vx, vy, vw, vh) = Self::virtual_screen();
            let (ax, ay) = Self::to_abs_virtualdesk(cx, cy, vx, vy, vw, vh);
            let ev = [INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: ax,
                        dy: ay,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            }];
            unsafe {
                SendInput(&ev, std::mem::size_of::<INPUT>() as i32);
            }
            return Ok(ToolResult::ok(format!("Hovered at ({}, {})", cx, cy)));
        }

        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);
        match matcher.find_first() {
            Ok(element) => {
                let rect = element
                    .get_bounding_rectangle()
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                let cx = (rect.get_left() + rect.get_right()) / 2;
                let cy = (rect.get_top() + rect.get_bottom()) / 2;
                use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
                unsafe {
                    let _ = SetCursorPos(cx, cy);
                }
                Ok(ToolResult::ok(format!(
                    "Hovered: '{}'",
                    element.get_name().unwrap_or_default()
                )))
            }
            Err(e) => Ok(ToolResult::err(format!(
                "Element not found for hover: {}",
                e
            ))),
        }
    }

    fn scroll_element(&self, input: &Value) -> Result<ToolResult> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL,
            MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT,
            MOUSE_EVENT_FLAGS,
        };

        let direction = input["direction"].as_str().unwrap_or("down");
        let amount = input["amount"].as_i64().unwrap_or(3) as i32;

        if let (Some(x), Some(y)) = (input["x"].as_i64(), input["y"].as_i64()) {
            // Move cursor using SendInput+VIRTUALDESK for multi-monitor support.
            let (raw_x, raw_y) = (x as i32, y as i32);
            let (cx, cy) = if input["_skip_calibration"].as_bool().unwrap_or(false) {
                (raw_x, raw_y)
            } else {
                crate::calibration::apply_active_calibration(raw_x, raw_y)
            };
            let (vx, vy, vw, vh) = Self::virtual_screen();
            let (ax, ay) = Self::to_abs_virtualdesk(cx, cy, vx, vy, vw, vh);
            let ev = [INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: ax,
                        dy: ay,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            }];
            unsafe {
                SendInput(&ev, std::mem::size_of::<INPUT>() as i32);
            }
        }

        let (flags, wheel_data): (MOUSE_EVENT_FLAGS, i32) = match direction {
            "up" => (MOUSEEVENTF_WHEEL, 120 * amount),
            "down" => (MOUSEEVENTF_WHEEL, -120 * amount),
            "left" => (MOUSEEVENTF_HWHEEL, -120 * amount),
            "right" => (MOUSEEVENTF_HWHEEL, 120 * amount),
            _ => (MOUSEEVENTF_WHEEL, -120 * amount),
        };

        let input_ev = [INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: wheel_data as u32,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }];
        unsafe {
            SendInput(&input_ev, std::mem::size_of::<INPUT>() as i32);
        }
        Ok(ToolResult::ok(format!(
            "Scrolled {} by {} ticks",
            direction, amount
        )))
    }

    /// Get the virtual desktop bounds in physical pixels.
    /// Returns (vx, vy, vw, vh) where (vx,vy) is the top-left origin (may be negative
    /// when a monitor is positioned to the left/above the primary).
    fn virtual_screen() -> (i32, i32, i32, i32) {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        };
        unsafe {
            let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            (vx, vy, vw, vh)
        }
    }

    /// Convert physical screen coordinates to SendInput absolute values (0-65535).
    /// Uses the full virtual desktop so coordinates on any monitor are valid.
    fn to_abs_virtualdesk(px: i32, py: i32, vx: i32, vy: i32, vw: i32, vh: i32) -> (i32, i32) {
        let ax = ((px - vx) * 65535 + vw / 2) / vw.max(1);
        let ay = ((py - vy) * 65535 + vh / 2) / vh.max(1);
        (ax, ay)
    }

    fn drag_drop(&self, input: &Value) -> Result<ToolResult> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
            MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT,
        };

        let (x1, y1) = match (input["x"].as_i64(), input["y"].as_i64()) {
            (Some(x), Some(y)) => (x as i32, y as i32),
            _ => {
                return Ok(ToolResult::err(
                    "drag_drop requires x, y (start) and x2, y2 (end)",
                ))
            }
        };
        let (x2, y2) = match (input["x2"].as_i64(), input["y2"].as_i64()) {
            (Some(x), Some(y)) => (x as i32, y as i32),
            _ => {
                return Ok(ToolResult::err(
                    "drag_drop requires x2, y2 (end coordinates)",
                ))
            }
        };

        // Coordinates are physical screen pixels (same as screen_capture grid labels).
        // Apply per-monitor manual calibration to both endpoints. The
        // start and end may live on different monitors, so we calibrate
        // each independently rather than translating the delta.
        let (cx1, cy1, cx2, cy2) = if input["_skip_calibration"].as_bool().unwrap_or(false) {
            (x1, y1, x2, y2)
        } else {
            let (a, b) = crate::calibration::apply_active_calibration(x1, y1);
            let (c, d) = crate::calibration::apply_active_calibration(x2, y2);
            (a, b, c, d)
        };

        let (vx, vy, vw, vh) = Self::virtual_screen();
        tracing::info!(
            "drag_drop: physical({},{})→({},{}) calibrated=({},{})→({},{}) virtual_screen=({},{})+({}x{})",
            x1, y1, x2, y2, cx1, cy1, cx2, cy2, vx, vy, vw, vh
        );

        let (ax1, ay1) = Self::to_abs_virtualdesk(cx1, cy1, vx, vy, vw, vh);
        let (ax2, ay2) = Self::to_abs_virtualdesk(cx2, cy2, vx, vy, vw, vh);
        tracing::info!("drag_drop: abs=({},{})→({},{})", ax1, ay1, ax2, ay2);

        // VIRTUALDESK: maps 0-65535 to the full virtual desktop (all monitors, physical pixels)
        let flags_move = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
        let flags_down = MOUSEEVENTF_LEFTDOWN | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
        let flags_up = MOUSEEVENTF_LEFTUP | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;

        let make_mouse_input = |dx: i32, dy: i32, flags| INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            // Move to start position
            let ev = [make_mouse_input(ax1, ay1, flags_move)];
            SendInput(&ev, std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(std::time::Duration::from_millis(80));

            // Press left button at start
            let ev = [make_mouse_input(ax1, ay1, flags_down)];
            SendInput(&ev, std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(std::time::Duration::from_millis(80));

            // Smoothly move to end position in steps
            let steps = 20i32;
            for i in 1..=steps {
                let ix = ax1 + (ax2 - ax1) * i / steps;
                let iy = ay1 + (ay2 - ay1) * i / steps;
                let ev = [make_mouse_input(ix, iy, flags_move)];
                SendInput(&ev, std::mem::size_of::<INPUT>() as i32);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            // Release left button at end
            let ev = [make_mouse_input(ax2, ay2, flags_up)];
            SendInput(&ev, std::mem::size_of::<INPUT>() as i32);
        }

        let calib_note = if (cx1, cy1, cx2, cy2) != (x1, y1, x2, y2) {
            format!(" [calibrated from ({},{})->({},{})]", x1, y1, x2, y2)
        } else {
            String::new()
        };
        Ok(ToolResult::ok(format!(
            "Dragged from ({},{}) to ({},{}){}",
            cx1, cy1, cx2, cy2, calib_note
        )))
    }

    // ─── Keyboard Actions ─────────────────────────────────────────────────────

    fn type_text(&self, input: &Value) -> Result<ToolResult> {
        let text = match input["text"].as_str() {
            Some(t) => t,
            None => return Ok(ToolResult::err("Missing parameter: text")),
        };

        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let mut matcher = automation.create_matcher().from(root).timeout(3000);
        if let Some(name) = input["name"].as_str() {
            matcher = matcher.name(name);
        }
        if let Some(class) = input["class_name"].as_str() {
            matcher = matcher.classname(class);
        }

        // Fail closed: never fall back to global keyboard input at the focused window.
        let target = matcher.find_first().ok();
        type_text_into(target, text, 10)
    }

    fn send_hotkey(&self, input: &Value) -> Result<ToolResult> {
        let hotkey = match input["hotkey"].as_str() {
            Some(h) => h.to_lowercase(),
            None => return Ok(ToolResult::err("Missing parameter: hotkey (e.g. 'ctrl+c')")),
        };

        use windows::Win32::UI::Input::KeyboardAndMouse::{
            keybd_event, KEYEVENTF_KEYUP, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END,
            VK_ESCAPE, VK_F1, VK_F10, VK_F11, VK_F12, VK_F2, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7,
            VK_F8, VK_F9, VK_HOME, VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN,
            VK_RIGHT, VK_SHIFT, VK_TAB, VK_UP,
        };

        let parts: Vec<&str> = hotkey.split('+').collect();
        let mut vkeys: Vec<u8> = Vec::new();

        for part in &parts {
            let vk: u8 = match part.trim() {
                "ctrl" | "control" => VK_CONTROL.0 as u8,
                "alt" => VK_MENU.0 as u8,
                "shift" => VK_SHIFT.0 as u8,
                "win" | "windows" => VK_LWIN.0 as u8,
                "enter" | "return" => VK_RETURN.0 as u8,
                "esc" | "escape" => VK_ESCAPE.0 as u8,
                "tab" => VK_TAB.0 as u8,
                "delete" | "del" => VK_DELETE.0 as u8,
                "backspace" => VK_BACK.0 as u8,
                "home" => VK_HOME.0 as u8,
                "end" => VK_END.0 as u8,
                "pageup" => VK_PRIOR.0 as u8,
                "pagedown" => VK_NEXT.0 as u8,
                "left" => VK_LEFT.0 as u8,
                "right" => VK_RIGHT.0 as u8,
                "up" => VK_UP.0 as u8,
                "down" => VK_DOWN.0 as u8,
                "f1" => VK_F1.0 as u8,
                "f2" => VK_F2.0 as u8,
                "f3" => VK_F3.0 as u8,
                "f4" => VK_F4.0 as u8,
                "f5" => VK_F5.0 as u8,
                "f6" => VK_F6.0 as u8,
                "f7" => VK_F7.0 as u8,
                "f8" => VK_F8.0 as u8,
                "f9" => VK_F9.0 as u8,
                "f10" => VK_F10.0 as u8,
                "f11" => VK_F11.0 as u8,
                "f12" => VK_F12.0 as u8,
                s if s.len() == 1 => s.chars().next().unwrap().to_ascii_uppercase() as u8,
                _ => continue,
            };
            vkeys.push(vk);
        }

        if vkeys.is_empty() {
            return Ok(ToolResult::err(format!(
                "Could not parse hotkey: {}",
                hotkey
            )));
        }

        // Fail closed: the hotkey must name its target window, which must be in the foreground,
        // with no protected window and no user takeover. Nothing is sent otherwise.
        if let Some(msg) = hotkey_target_refusal(input) {
            return Ok(ToolResult::err(msg));
        }
        let expected = input_guard::Target::from_hwnd(self.find_window_hwnd(input)?);
        if let Err(msg) = input_guard::check(Some(&expected)) {
            return Ok(ToolResult::err(msg));
        }

        unsafe {
            // Press all keys
            for &vk in &vkeys {
                keybd_event(
                    vk,
                    0,
                    windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                    0,
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            // Release all keys in reverse
            for &vk in vkeys.iter().rev() {
                keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        Ok(ToolResult::ok(format!("Sent hotkey: {}", hotkey)))
    }

    fn send_keys_action(&self, input: &Value) -> Result<ToolResult> {
        let keys = match input["keys"].as_str() {
            Some(k) => k,
            None => return Ok(ToolResult::err("Missing parameter: keys")),
        };

        // Keys are only sent to a named target element. There is no focused-window fallback.
        if input["name"].as_str().is_none()
            && input["class_name"].as_str().is_none()
            && input["automation_id"].as_str().is_none()
        {
            return Ok(ToolResult::err(
                "send_keys refused: give name, class_name or automation_id of the target element (keys are never sent to the focused window)",
            ));
        }
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 3000);
        let target = matcher.find_first().ok();
        type_text_into(target, keys, 50)
    }

    // ─── State Actions ────────────────────────────────────────────────────────

    fn expand_element(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::patterns::UIExpandCollapsePattern;
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);
        match matcher.find_first() {
            Ok(element) => {
                let name = element.get_name().unwrap_or_default();
                // Try ExpandCollapsePattern first (correct semantic)
                if let Ok(pattern) = element.get_pattern::<UIExpandCollapsePattern>() {
                    pattern.expand().map_err(|e| anyhow::anyhow!("{}", e))?;
                    return Ok(ToolResult::ok(format!(
                        "Expanded '{}' via ExpandCollapsePattern",
                        name
                    )));
                }
                // Fallback: click (toggles state, may not guarantee expanded)
                element.click().map_err(|e| anyhow::anyhow!("{}", e))?;
                Ok(ToolResult::ok(format!(
                    "Expanded (clicked fallback): '{}'",
                    name
                )))
            }
            Err(e) => Ok(ToolResult::err(format!("Element not found: {}", e))),
        }
    }

    fn collapse_element(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::patterns::UIExpandCollapsePattern;
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);
        match matcher.find_first() {
            Ok(element) => {
                let name = element.get_name().unwrap_or_default();
                // Try ExpandCollapsePattern first (correct semantic)
                if let Ok(pattern) = element.get_pattern::<UIExpandCollapsePattern>() {
                    pattern.collapse().map_err(|e| anyhow::anyhow!("{}", e))?;
                    return Ok(ToolResult::ok(format!(
                        "Collapsed '{}' via ExpandCollapsePattern",
                        name
                    )));
                }
                // Fallback: click
                element.click().map_err(|e| anyhow::anyhow!("{}", e))?;
                Ok(ToolResult::ok(format!(
                    "Collapsed (clicked fallback): '{}'",
                    name
                )))
            }
            Err(e) => Ok(ToolResult::err(format!("Element not found: {}", e))),
        }
    }

    fn select_item(&self, input: &Value) -> Result<ToolResult> {
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);
        match matcher.find_first() {
            Ok(element) => {
                element.click().map_err(|e| anyhow::anyhow!("{}", e))?;
                Ok(ToolResult::ok(format!(
                    "Selected (clicked): '{}'",
                    element.get_name().unwrap_or_default()
                )))
            }
            Err(e) => Ok(ToolResult::err(format!("Element not found: {}", e))),
        }
    }

    fn set_check(&self, input: &Value, checked: bool) -> Result<ToolResult> {
        use uiautomation::UIAutomation;
        let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let root = self.get_search_root(&automation, input)?;
        let matcher = self.build_matcher(&automation, root, input, 5000);
        match matcher.find_first() {
            Ok(element) => {
                // Click to toggle checkbox state
                element.click().map_err(|e| anyhow::anyhow!("{}", e))?;
                let action = if checked { "Checked" } else { "Unchecked" };
                Ok(ToolResult::ok(format!(
                    "{} (clicked): '{}'",
                    action,
                    element.get_name().unwrap_or_default()
                )))
            }
            Err(e) => Ok(ToolResult::err(format!("Element not found: {}", e))),
        }
    }

    // ─── Wait ─────────────────────────────────────────────────────────────────

    async fn wait_for_element(&self, input: &Value, ctx: &ToolContext) -> Result<ToolResult> {
        use uiautomation::UIAutomation;
        let timeout_ms = input["timeout_ms"].as_u64().unwrap_or(10000);
        let poll_ms = 500u64;
        let start = std::time::Instant::now();

        loop {
            if ctx.is_cancelled() {
                return Ok(ToolResult::err("已被用户取消"));
            }

            let found_name = {
                let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
                match self.get_search_root(&automation, input) {
                    Ok(root) => {
                        let matcher = self.build_matcher(&automation, root, input, 1000);
                        matcher
                            .find_first()
                            .ok()
                            .map(|element| element.get_name().unwrap_or_default())
                    }
                    Err(_) => None,
                }
            };

            if let Some(name) = found_name {
                let elapsed = start.elapsed().as_millis();
                return Ok(ToolResult::ok(format!(
                    "Element found after {}ms: Name='{}'",
                    elapsed, name
                )));
            }

            if start.elapsed().as_millis() as u64 >= timeout_ms {
                return Ok(ToolResult::err(format!(
                    "Timeout after {}ms: element not found",
                    timeout_ms
                )));
            }

            tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
        }
    }

    // ─── Window Management ────────────────────────────────────────────────────

    fn find_window_hwnd(&self, input: &Value) -> Result<windows::Win32::Foundation::HWND> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, FindWindowW, GetForegroundWindow, GetWindowTextW, IsWindowVisible,
        };

        let title = match input["name"].as_str().or(input["window_title"].as_str()) {
            Some(t) => t,
            None => return Ok(unsafe { GetForegroundWindow() }),
        };

        // First try exact match (fast path)
        let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        if let Ok(hwnd) = unsafe { FindWindowW(PCWSTR::null(), PCWSTR(wide.as_ptr())) } {
            return Ok(hwnd);
        }

        // Fallback: partial match via EnumWindows (consistent with get_search_root)
        let title_lower = title.to_lowercase();
        #[allow(clippy::arc_with_non_send_sync)]
        let result = std::sync::Arc::new(std::sync::Mutex::new(None::<HWND>));
        let result_clone = result.clone();
        let title_clone = title_lower.clone();

        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let data =
                &*(lparam.0 as *const (String, std::sync::Arc<std::sync::Mutex<Option<HWND>>>));
            let (title_lower, result) = data;
            if IsWindowVisible(hwnd).as_bool() {
                let mut buf = [0u16; 512];
                let len = GetWindowTextW(hwnd, &mut buf);
                if len > 0 {
                    let window_title =
                        String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
                    if window_title.contains(title_lower.as_str()) {
                        *result.lock().unwrap() = Some(hwnd);
                        return BOOL(0); // stop enumeration
                    }
                }
            }
            BOOL(1) // continue
        }

        let data = (title_clone, result_clone);
        let _ = unsafe { EnumWindows(Some(enum_callback), LPARAM(&data as *const _ as isize)) };

        let found = *result.lock().unwrap();
        match found {
            Some(hwnd) => Ok(hwnd),
            None => Err(anyhow::anyhow!(
                "Window '{}' not found (tried exact and partial match)",
                title
            )),
        }
    }

    fn activate_window(&self, input: &Value) -> Result<ToolResult> {
        use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows::Win32::UI::Input::KeyboardAndMouse::SetActiveWindow;
        use windows::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
            IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
        };

        let hwnd = self.find_window_hwnd(input)?;

        unsafe {
            // 1. Restore if minimized
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                std::thread::sleep(std::time::Duration::from_millis(200));
            } else {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }

            // 2. Already foreground — nothing to do
            if GetForegroundWindow() == hwnd {
                return Ok(ToolResult::ok("Window already in foreground"));
            }

            // 3. AttachThreadInput: the most reliable way to steal foreground
            //    without sending global keyboard events (which would disturb
            //    other windows like the IDE).
            let fg_hwnd = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg_hwnd, None);
            let our_thread = GetCurrentThreadId();
            let attached = fg_thread != 0 && fg_thread != our_thread;
            if attached {
                let _ = AttachThreadInput(our_thread, fg_thread, true);
            }

            let _ = BringWindowToTop(hwnd);
            let _ = SetActiveWindow(hwnd);
            let _ = SetForegroundWindow(hwnd);

            if attached {
                let _ = AttachThreadInput(our_thread, fg_thread, false);
            }

            // Give the OS time to process the foreground change.
            std::thread::sleep(std::time::Duration::from_millis(300));

            if GetForegroundWindow() != hwnd && IsWindowVisible(hwnd).as_bool() {
                return Ok(ToolResult::ok(
                    "Window activated (best-effort; foreground lock may be held by another process)"
                ));
            }
        }
        Ok(ToolResult::ok("Window activated and in foreground"))
    }

    fn window_state(&self, input: &Value, state: &str) -> Result<ToolResult> {
        use windows::Win32::UI::WindowsAndMessaging::{
            ShowWindow, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE,
        };
        let hwnd = self.find_window_hwnd(input)?;
        let cmd = match state {
            "minimize" => SW_MINIMIZE,
            "maximize" => SW_MAXIMIZE,
            "restore" => SW_RESTORE,
            _ => SW_RESTORE,
        };
        unsafe {
            let _ = ShowWindow(hwnd, cmd);
        }
        Ok(ToolResult::ok(format!("Window {}", state)))
    }

    fn close_window(&self, input: &Value) -> Result<ToolResult> {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
        let hwnd = self.find_window_hwnd(input)?;
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0))
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Ok(ToolResult::ok("Window close message sent"))
    }

    fn move_window(&self, input: &Value) -> Result<ToolResult> {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOP, SWP_NOSIZE, SWP_NOZORDER,
        };
        let hwnd = self.find_window_hwnd(input)?;
        let x = input["x"].as_i64().unwrap_or(0) as i32;
        let y = input["y"].as_i64().unwrap_or(0) as i32;
        unsafe {
            SetWindowPos(hwnd, HWND_TOP, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Ok(ToolResult::ok(format!("Window moved to ({}, {})", x, y)))
    }

    fn resize_window(&self, input: &Value) -> Result<ToolResult> {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOP, SWP_NOMOVE, SWP_NOZORDER,
        };
        let hwnd = self.find_window_hwnd(input)?;
        let w = input["x2"].as_i64().unwrap_or(800) as i32;
        let h = input["y2"].as_i64().unwrap_or(600) as i32;
        unsafe {
            SetWindowPos(hwnd, HWND_TOP, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Ok(ToolResult::ok(format!("Window resized to {}x{}", w, h)))
    }

    fn get_window_rect(&self, input: &Value) -> Result<ToolResult> {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        let hwnd = self.find_window_hwnd(input)?;
        let mut rect = RECT::default();
        unsafe {
            GetWindowRect(hwnd, &mut rect).map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Ok(ToolResult::ok(format!(
            "Window rect: left={}, top={}, right={}, bottom={}, width={}, height={}",
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            rect.right - rect.left,
            rect.bottom - rect.top
        )))
    }

    // ─── Hybrid Vision Automation ────────────────────────────────────────────

    async fn smart_find(&self, input: &Value, _ctx: &ToolContext) -> Result<ToolResult> {
        // Phase 1: synchronous UIA work — collect all data, then drop all
        // uiautomation references before any .await (they are not Send).
        let description = input["description"].as_str().unwrap_or("").to_string();

        #[allow(clippy::type_complexity)]
        let uia_result: Result<Option<(String, String, i32, i32, i32, i32)>> = (|| {
            use uiautomation::UIAutomation;
            let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
            let root = self.get_search_root(&automation, input)?;

            let mut matcher = automation.create_matcher().from(root.clone()).timeout(3000);
            if let Some(name) = input["name"].as_str() {
                matcher = matcher.name(name);
            } else if !description.is_empty() {
                matcher = matcher.name(&description);
            }
            if let Some(class) = input["class_name"].as_str() {
                matcher = matcher.classname(class);
            }
            if let Some(ct) = input["control_type"].as_str() {
                matcher = matcher.classname(ct);
            }

            match matcher.find_first() {
                Ok(element) => {
                    let name = element.get_name().unwrap_or_default();
                    let class = element.get_classname().unwrap_or_default();
                    let rect = element
                        .get_bounding_rectangle()
                        .map_err(|e| anyhow::anyhow!("{}", e))?;
                    Ok(Some((
                        name,
                        class,
                        rect.get_left(),
                        rect.get_top(),
                        rect.get_right(),
                        rect.get_bottom(),
                    )))
                }
                Err(_) => Ok(None),
            }
        })();

        match uia_result? {
            Some((name, class, left, top, right, bottom)) => {
                let cx = (left + right) / 2;
                let cy = (top + bottom) / 2;
                Ok(ToolResult::ok(format!(
                    "Found via UIA: Name='{}', Class='{}', Center=({}, {}), Rect=[{},{},{},{}]",
                    name, class, cx, cy, left, top, right, bottom
                )))
            }
            None => {
                let screen = crate::screen::ScreenTool;
                let capture_input = serde_json::json!({
                    "action": "capture",
                    "format": "jpeg",
                    "quality": 75
                });
                let ctx_clone = _ctx.clone();
                match screen.call(capture_input, &ctx_clone).await {
                    Ok(result) => {
                        let msg = format!(
                            "UIA could not find element matching '{}'. Screenshot captured for Vision AI analysis. \
                             Please analyze the screenshot to locate the element and provide coordinates.",
                            description
                        );
                        if let Some(img) = result.image {
                            Ok(ToolResult::ok(msg).with_image(img))
                        } else {
                            Ok(ToolResult::ok(msg))
                        }
                    }
                    Err(e) => Ok(ToolResult::err(format!(
                        "UIA search failed and screenshot also failed: {}",
                        e
                    ))),
                }
            }
        }
    }

    async fn annotate_elements(&self, input: &Value, _ctx: &ToolContext) -> Result<ToolResult> {
        // Phase 1: synchronous UIA work — collect all data in a closure so
        // that all uiautomation references (which are not Send) are dropped
        // before we .await the screen capture.
        let elements: Vec<(String, String, i32, i32, i32, i32)> = {
            use uiautomation::UIAutomation;

            let automation = UIAutomation::new().map_err(|e| anyhow::anyhow!("{}", e))?;
            let root = self.get_search_root(&automation, input)?;
            let max_elements = input["max_elements"].as_u64().unwrap_or(30) as usize;

            let walker = automation
                .get_control_view_walker()
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let mut elems: Vec<(String, String, i32, i32, i32, i32)> = Vec::new();
            self.collect_interactive_elements(&walker, &root, 0, 4, &mut elems, max_elements);
            elems
        }; // automation / root / walker dropped here

        if elements.is_empty() {
            return Ok(ToolResult::err(
                "No interactive elements found in the target window",
            ));
        }

        let screen = crate::screen::ScreenTool;
        let capture_input = serde_json::json!({ "action": "capture", "format": "png" });
        let ctx_clone = _ctx.clone();
        let capture_result = screen.call(capture_input, &ctx_clone).await?;

        let mut map_text = String::from("Annotated elements:\n");
        for (i, (name, class, left, top, right, bottom)) in elements.iter().enumerate() {
            let cx = (left + right) / 2;
            let cy = (top + bottom) / 2;
            map_text.push_str(&format!(
                "[{}] Name='{}', Class='{}', Center=({},{}), Rect=[{},{},{},{}]\n",
                i + 1,
                name,
                class,
                cx,
                cy,
                left,
                top,
                right,
                bottom
            ));
        }

        if let Some(img) = capture_result.image {
            Ok(ToolResult::ok(map_text).with_image(img))
        } else {
            Ok(ToolResult::ok(map_text))
        }
    }

    fn collect_interactive_elements(
        &self,
        walker: &uiautomation::UITreeWalker,
        element: &uiautomation::UIElement,
        depth: usize,
        max_depth: usize,
        results: &mut Vec<(String, String, i32, i32, i32, i32)>,
        max_elements: usize,
    ) {
        if depth >= max_depth || results.len() >= max_elements {
            return;
        }
        if let Ok(child) = walker.get_first_child(element) {
            let mut current = child;
            loop {
                if results.len() >= max_elements {
                    break;
                }
                let name = current.get_name().unwrap_or_default();
                let class = current.get_classname().unwrap_or_default();

                let is_interactive = matches!(
                    class.as_str(),
                    "Button"
                        | "Edit"
                        | "ComboBox"
                        | "CheckBox"
                        | "RadioButton"
                        | "ListItem"
                        | "MenuItem"
                        | "TabItem"
                        | "Hyperlink"
                        | "TreeItem"
                        | "Slider"
                        | "Spinner"
                        | "ToggleButton"
                        | "SplitButton"
                ) || class.contains("Button")
                    || class.contains("Edit")
                    || class.contains("TextBox");

                if is_interactive {
                    if let Ok(rect) = current.get_bounding_rectangle() {
                        let l = rect.get_left();
                        let t = rect.get_top();
                        let r = rect.get_right();
                        let b = rect.get_bottom();
                        if r > l && b > t && (r - l) > 2 && (b - t) > 2 {
                            results.push((name, class, l, t, r, b));
                        }
                    }
                }

                self.collect_interactive_elements(
                    walker,
                    &current,
                    depth + 1,
                    max_depth,
                    results,
                    max_elements,
                );
                match walker.get_next_sibling(&current) {
                    Ok(next) => current = next,
                    Err(_) => break,
                }
            }
        }
    }
}

/// Target of a keyboard request. Abstracted so the fail-closed decision can be
/// unit-tested without a live UIA tree.
trait TypeTextTarget {
    /// True when the element can accept keyboard focus and is not a password field.
    fn accepts_keyboard_input(&self) -> bool;
    /// Verify the target window is foreground and no user takeover happened. Must pass before any input.
    fn preflight(&self) -> Result<(), String>;
    /// Send `text` to the element with `interval_ms` between keys. Only called after `preflight`.
    fn send_text(&self, text: &str, interval_ms: u64) -> Result<()>;
}

impl TypeTextTarget for uiautomation::UIElement {
    fn accepts_keyboard_input(&self) -> bool {
        // Unknown password state is treated as a password field (fail closed).
        self.is_keyboard_focusable().unwrap_or(false) && !self.is_password().unwrap_or(true)
    }

    fn preflight(&self) -> Result<(), String> {
        let target = input_guard::target_from_element(self)?;
        input_guard::check(Some(&target))
    }

    fn send_text(&self, text: &str, interval_ms: u64) -> Result<()> {
        self.send_keys(text, interval_ms)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }
}

/// Refusal for a hotkey that names no target window. Hotkeys are never sent to whichever
/// window happens to be focused, so `window_title` (or `name`) is required.
fn hotkey_target_refusal(input: &Value) -> Option<&'static str> {
    // Same precedence as find_window_hwnd (name, then window_title). An empty title counts as
    // missing: a partial match on "" would hit the first visible window.
    let title = input["name"].as_str().or(input["window_title"].as_str());
    if title.map(str::trim).is_some_and(|t| !t.is_empty()) {
        None
    } else {
        Some("send_hotkey refused: give window_title (or name) of the target window (hotkeys are never sent to the focused window)")
    }
}

/// Fail-closed core of keyboard input. `target` is `None` when the element could not be
/// resolved. Input is only sent to a resolved, keyboard-focusable, non-password element whose
/// window is in the foreground, with no protected window and no user takeover.
fn type_text_into<T: TypeTextTarget>(target: Option<T>, text: &str, interval_ms: u64) -> Result<ToolResult> {
    let target = match target {
        Some(t) => t,
        None => {
            return Ok(ToolResult::err(
                "refused: target element not found (no input sent to the focused window)",
            ))
        }
    };
    if !target.accepts_keyboard_input() {
        return Ok(ToolResult::err(
            "refused: target element is not keyboard-focusable or is a password field (no input sent)",
        ));
    }
    if let Err(msg) = target.preflight() {
        return Ok(ToolResult::err(msg));
    }
    target.send_text(text, interval_ms)?;
    Ok(ToolResult::ok("Sent input to target element"))
}

/// Keyboard-injection guards for the Windows UIA tool.
///
/// Every injected keystroke passes `check` (foreground window equals the target, no protected
/// window in front, no user takeover since the last successful agent input). Successful
/// injections call `mark_agent_input`, so later user input can be told apart from our own.
pub(crate) mod input_guard {
    use std::sync::atomic::{AtomicU32, Ordering};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetAncestor, GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, GA_ROOT,
    };

    /// Actions that inject input into the desktop. Only these advance the agent-input baseline.
    pub(crate) const INJECTING_ACTIONS: &[&str] = &[
        "click",
        "double_click",
        "right_click",
        "hover",
        "scroll",
        "drag_drop",
        "type",
        "send_hotkey",
        "send_keys",
        "expand",
        "collapse",
        "select",
        "check",
        "uncheck",
    ];

    /// GetTickCount value of the most recent successful injection by this tool; 0 means none yet.
    static LAST_AGENT_INPUT_TICK: AtomicU32 = AtomicU32::new(0);
    /// Input observed within this window after an injection is attributed to that injection.
    const INJECTION_ECHO_GRACE_MS: u32 = 100;
    /// Window classes that must never receive injected input (credential prompts).
    const PROTECTED_WINDOW_CLASSES: &[&str] =
        &["Credential Dialog Xaml Host", "CredUIBrokerWindow"];

    /// Identity of the window an action intends to act on.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct Target {
        /// Top-level window handle (0 when unknown).
        pub root_hwnd: isize,
        /// Owning process id (0 when unknown).
        pub pid: u32,
    }

    impl Target {
        pub(crate) fn from_hwnd(hwnd: HWND) -> Self {
            if hwnd.0.is_null() {
                return Target { root_hwnd: 0, pid: 0 };
            }
            let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
            Target {
                root_hwnd: root.0 as isize,
                pid: pid_of(root),
            }
        }
    }

    fn pid_of(hwnd: HWND) -> u32 {
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        pid
    }

    /// Resolve the top-level window of a UIA element. Refuses when neither handle nor pid is known.
    pub(crate) fn target_from_element(el: &uiautomation::UIElement) -> Result<Target, String> {
        let from_handle = match el.get_native_window_handle() {
            Ok(h) if !h.is_invalid() => {
                let raw: isize = h.into();
                Target::from_hwnd(HWND(raw as *mut std::ffi::c_void))
            }
            _ => Target { root_hwnd: 0, pid: 0 },
        };
        let pid = if from_handle.pid != 0 {
            from_handle.pid
        } else {
            el.get_process_id().unwrap_or(0)
        };
        if from_handle.root_hwnd == 0 && pid == 0 {
            return Err("refused: cannot identify the target window (no input sent)".to_string());
        }
        Ok(Target {
            root_hwnd: from_handle.root_hwnd,
            pid,
        })
    }

    struct Foreground {
        hwnd: isize,
        pid: u32,
        class: String,
    }

    fn foreground() -> Foreground {
        let fg = unsafe { GetForegroundWindow() };
        if fg.0.is_null() {
            return Foreground {
                hwnd: 0,
                pid: 0,
                class: String::new(),
            };
        }
        let mut buf = [0u16; 256];
        let n = unsafe { GetClassNameW(fg, &mut buf) };
        let len = (n.max(0) as usize).min(buf.len());
        Foreground {
            hwnd: fg.0 as isize,
            pid: pid_of(fg),
            class: String::from_utf16_lossy(&buf[..len]),
        }
    }

    /// Tick of the last user keyboard or mouse input, or None if the OS will not say.
    fn last_user_input_tick() -> Option<u32> {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if unsafe { GetLastInputInfo(&mut info) }.as_bool() {
            Some(info.dwTime)
        } else {
            None
        }
    }

    /// Record that this tool just injected input. Call only after a successful injection.
    pub(crate) fn mark_agent_input() {
        let now = unsafe { GetTickCount() }.max(1);
        LAST_AGENT_INPUT_TICK.store(now, Ordering::SeqCst);
    }

    /// Preflight for one injection. `expected` is the window the action targets; `None` means global input.
    pub(crate) fn check(expected: Option<&Target>) -> Result<(), String> {
        let fg = foreground();
        let user_tick = last_user_input_tick()
            .ok_or_else(|| "refused: cannot read last input time (no input sent)".to_string())?;
        decide(
            fg.hwnd,
            fg.pid,
            &fg.class,
            expected,
            user_tick,
            LAST_AGENT_INPUT_TICK.load(Ordering::SeqCst),
        )
    }

    /// Pure decision behind `check`, separated so the policy can be unit-tested.
    pub(crate) fn decide(
        fg_hwnd: isize,
        fg_pid: u32,
        fg_class: &str,
        expected: Option<&Target>,
        user_tick: u32,
        agent_tick: u32,
    ) -> Result<(), String> {
        if fg_hwnd == 0 {
            return Err("focus lost: no foreground window; stopped without sending input".to_string());
        }
        if let Some(t) = expected {
            let same_window = if t.root_hwnd != 0 {
                fg_hwnd == t.root_hwnd
            } else {
                t.pid != 0 && fg_pid == t.pid
            };
            if !same_window {
                return Err(
                    "focus lost: the target window is not in the foreground; stopped without sending input. Call activate_window on the target first."
                        .to_string(),
                );
            }
        }
        if PROTECTED_WINDOW_CLASSES.contains(&fg_class) {
            return Err(
                "refused: a credential or secure window is in the foreground (no input sent)"
                    .to_string(),
            );
        }
        // A user input later than the last agent input means the user took over.
        // The wrapping difference keeps the comparison correct across GetTickCount wrap-around.
        if agent_tick != 0 {
            let since = user_tick.wrapping_sub(agent_tick);
            if since > INJECTION_ECHO_GRACE_MS && since < 0x8000_0000 {
                return Err(
                    "user input detected since the last agent input; stopped without sending input"
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod type_text_tests {
    use super::input_guard::{decide, Target};
    use super::*;
    use std::cell::RefCell;

    /// Fake element: records every `send_text` call (the keyboard-injection point).
    struct FakeElement {
        focusable: bool,
        preflight: Result<(), String>,
        sent: RefCell<Vec<String>>,
    }

    impl FakeElement {
        fn new(focusable: bool, preflight: Result<(), String>) -> Self {
            Self {
                focusable,
                preflight,
                sent: RefCell::new(Vec::new()),
            }
        }
    }

    impl TypeTextTarget for &FakeElement {
        fn accepts_keyboard_input(&self) -> bool {
            self.focusable
        }

        fn preflight(&self) -> Result<(), String> {
            self.preflight.clone()
        }

        fn send_text(&self, text: &str, _interval_ms: u64) -> Result<()> {
            self.sent.borrow_mut().push(text.to_string());
            Ok(())
        }
    }

    #[test]
    fn unresolved_target_is_error_and_injects_nothing() {
        let result = type_text_into::<&FakeElement>(None, "secret", 10).unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }

    #[test]
    fn element_without_keyboard_support_is_error_and_injects_nothing() {
        let el = FakeElement::new(false, Ok(()));
        let result = type_text_into(Some(&el), "secret", 10).unwrap();
        assert!(result.is_error);
        assert!(el.sent.borrow().is_empty(), "keyboard injector must not be invoked");
    }

    #[test]
    fn hotkey_without_target_window_is_refused() {
        use serde_json::json;
        assert!(super::hotkey_target_refusal(&json!({ "hotkey": "ctrl+c" })).is_some());
        assert!(super::hotkey_target_refusal(&json!({ "hotkey": "ctrl+c", "window_title": "" })).is_some());
        assert!(super::hotkey_target_refusal(&json!({ "hotkey": "ctrl+c", "window_title": "   " })).is_some());
        // find_window_hwnd prefers `name`, so an empty name refuses even with a real window_title.
        assert!(super::hotkey_target_refusal(&json!({ "hotkey": "ctrl+c", "name": "", "window_title": "Notepad" })).is_some());
        assert!(super::hotkey_target_refusal(&json!({ "hotkey": "ctrl+c", "window_title": "Notepad" })).is_none());
        assert!(super::hotkey_target_refusal(&json!({ "hotkey": "ctrl+c", "name": "Notepad" })).is_none());
    }

    #[test]
    fn failed_preflight_is_error_and_injects_nothing() {
        let el = FakeElement::new(true, Err("focus lost".to_string()));
        let result = type_text_into(Some(&el), "secret", 10).unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("focus lost"));
        assert!(el.sent.borrow().is_empty(), "no input may follow a failed preflight");
    }

    #[test]
    fn supported_element_receives_text_once() {
        let el = FakeElement::new(true, Ok(()));
        let result = type_text_into(Some(&el), "hello", 10).unwrap();
        assert!(!result.is_error);
        assert_eq!(*el.sent.borrow(), vec!["hello".to_string()]);
    }

    const TARGET: Target = Target {
        root_hwnd: 100,
        pid: 7,
    };

    #[test]
    fn foreground_mismatch_is_focus_lost() {
        let r = decide(200, 9, "Notepad", Some(&TARGET), 0, 0);
        assert!(r.unwrap_err().starts_with("focus lost"));
    }

    #[test]
    fn no_foreground_window_is_refused() {
        assert!(decide(0, 0, "", Some(&TARGET), 0, 0).is_err());
        assert!(decide(0, 0, "", None, 0, 0).is_err());
    }

    #[test]
    fn matching_foreground_is_allowed() {
        assert!(decide(100, 1, "Notepad", Some(&TARGET), 0, 0).is_ok());
    }

    #[test]
    fn pid_fallback_matches_when_handle_unknown() {
        let t = Target { root_hwnd: 0, pid: 7 };
        assert!(decide(300, 7, "Edit", Some(&t), 0, 0).is_ok());
        assert!(decide(300, 8, "Edit", Some(&t), 0, 0).is_err());
        // Unknown pid and handle: never a match.
        let unknown = Target { root_hwnd: 0, pid: 0 };
        assert!(decide(300, 0, "Edit", Some(&unknown), 0, 0).is_err());
    }

    #[test]
    fn protected_window_is_refused() {
        let r = decide(100, 1, "Credential Dialog Xaml Host", Some(&TARGET), 0, 0);
        assert!(r.unwrap_err().contains("credential"));
        assert!(decide(100, 1, "CredUIBrokerWindow", None, 0, 0).is_err());
    }

    #[test]
    fn user_input_after_agent_input_stops() {
        // agent injected at tick 1000; user acted at 1500 (well past the echo grace)
        let r = decide(100, 1, "Notepad", Some(&TARGET), 1500, 1000);
        assert!(r.unwrap_err().contains("user input"));
    }

    #[test]
    fn echo_of_own_input_is_not_user_input() {
        assert!(decide(100, 1, "Notepad", Some(&TARGET), 1050, 1000).is_ok());
    }

    #[test]
    fn user_input_before_agent_input_is_ignored() {
        assert!(decide(100, 1, "Notepad", Some(&TARGET), 900, 1000).is_ok());
    }

    #[test]
    fn no_prior_agent_input_means_no_takeover_check() {
        assert!(decide(100, 1, "Notepad", Some(&TARGET), 5000, 0).is_ok());
    }

    #[test]
    fn takeover_check_survives_tick_wraparound() {
        // agent at u32::MAX - 10, user at 500 after wrap
        assert!(decide(100, 1, "Notepad", Some(&TARGET), 500, u32::MAX - 10).unwrap_err().contains("user input"));
    }
}
