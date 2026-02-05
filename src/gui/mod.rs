//! GUI definition using Slint

pub mod state;

slint::slint! {
    import { Button, VerticalBox, HorizontalBox, CheckBox, ListView, LineEdit, StandardTableView, ComboBox } from "std-widgets.slint";

    // Tree node for hierarchical display
    export struct TreeNode {
        path: string,           // Full path
        name: string,           // Display name (just filename or folder name)
        depth: int,             // Indentation level
        is_folder: bool,        // Is this a folder?
        expanded: bool,         // Is folder expanded?
        selected: bool,         // Is this item selected for extraction?
        partially_selected: bool, // Some children selected (indeterminate state)
        visible: bool,          // Is this visible (based on parent expansion + search)?
        has_children: bool,     // Does this folder have children?
        index: int,             // Index in flat list
    }

    component TreeRow inherits Rectangle {
        in property <TreeNode> node;
        in property <bool> odd_row;
        callback toggle_expand(int);
        callback toggle_select(int);

        height: node.visible ? 22px : 0px;
        background: odd_row ? #2a2a2a : #252525;

        if node.visible: HorizontalLayout {
            padding-left: (node.depth * 16px) + 4px;
            padding-right: 8px;
            spacing: 2px;
            alignment: start;

            // Expand/collapse arrow for folders
            Rectangle {
                width: 16px;
                horizontal-stretch: 0;

                if node.is_folder && node.has_children: TouchArea {
                    clicked => { toggle_expand(node.index); }

                    Text {
                        text: node.expanded ? "▼" : "▶";
                        font-size: 10px;
                        color: #aaaaaa;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                }
            }

            // Tri-state checkbox
            Rectangle {
                width: 20px;
                height: 16px;
                horizontal-stretch: 0;

                Rectangle {
                    x: 2px;
                    y: 2px;
                    width: 12px;
                    height: 12px;
                    border-radius: 2px;
                    border-width: 1px;
                    border-color: node.selected || node.partially_selected ? #4a9eff : #888888;
                    background: node.selected || node.partially_selected ? #4a9eff : transparent;

                    Text {
                        text: node.selected ? "✓" : node.partially_selected ? "−" : "";
                        font-size: 10px;
                        color: #ffffff;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                }

                TouchArea {
                    clicked => { toggle_select(node.index); }
                }
            }

            // Name - gets all remaining space
            Text {
                text: node.name;
                vertical-alignment: center;
                horizontal-alignment: left;
                color: node.is_folder ? #70b0ff : #e0e0e0;
                font-weight: node.is_folder ? 600 : 400;
                overflow: elide;
                horizontal-stretch: 1;
            }
        }
    }

    export component MainWindow inherits Window {
        title: "BSA/BA2 Archive Tool";
        min-width: 400px;
        min-height: 300px;
        preferred-width: 500px;
        preferred-height: 600px;
        background: #1e1e1e;

        // Properties
        in-out property <[TreeNode]> tree_nodes: [];
        in-out property <string> search_text: "";
        in-out property <string> window_title: "BSA/BA2 Archive Tool";
        in-out property <string> status_text: "";
        in-out property <float> progress: 0.0;
        in-out property <bool> is_processing: false;
        in-out property <bool> pack_mode: false;
        in-out property <[string]> game_versions: [];
        in-out property <int> selected_game_version: 0;

        // Callbacks
        callback open_file();
        callback open_folder();
        callback extract();
        callback pack();
        callback select_all();
        callback select_none();
        callback search_changed(string);
        callback toggle_expand(int);
        callback toggle_select(int);


        VerticalLayout {
            // Menu bar
            Rectangle {
                height: 28px;
                background: #2d2d2d;

                HorizontalLayout {
                    padding: 4px;
                    spacing: 0px;

                    file_menu := Rectangle {
                        width: 40px;
                        height: 20px;
                        background: file_touch.has-hover ? #3d3d3d : transparent;

                        file_touch := TouchArea {
                            clicked => {
                                file_popup.show();
                            }
                        }

                        Text {
                            text: "File";
                            horizontal-alignment: center;
                            vertical-alignment: center;
                            font-size: 12px;
                            color: #e0e0e0;
                        }
                    }

                    about_menu := Rectangle {
                        width: 50px;
                        height: 20px;
                        background: about_touch.has-hover ? #3d3d3d : transparent;

                        about_touch := TouchArea {
                            clicked => {
                                about_popup.show();
                            }
                        }

                        Text {
                            text: "About";
                            horizontal-alignment: center;
                            vertical-alignment: center;
                            font-size: 12px;
                            color: #e0e0e0;
                        }
                    }

                    Rectangle { horizontal-stretch: 1; }
                }
            }

            // Toolbar
            Rectangle {
                height: 36px;
                background: #2d2d2d;

                HorizontalLayout {
                    padding: 4px;
                    spacing: 8px;

                    Button {
                        text: "Select All";
                        clicked => { select_all(); }
                    }
                    Button {
                        text: "Select None";
                        clicked => { select_none(); }
                    }

                    Rectangle { horizontal-stretch: 1; }

                    if pack_mode: HorizontalLayout {
                        spacing: 4px;
                        alignment: end;

                        Text {
                            text: "Game:";
                            vertical-alignment: center;
                            font-size: 12px;
                            color: #aaaaaa;
                        }

                        ComboBox {
                            width: 240px;
                            model: game_versions;
                            current-index <=> selected_game_version;
                        }
                    }
                }
            }

            // Search bar
            Rectangle {
                height: 32px;
                background: #2d2d2d;

                HorizontalLayout {
                    padding: 4px;
                    spacing: 8px;

                    LineEdit {
                        horizontal-stretch: 1;
                        placeholder-text: "Search (use * for wildcard)";
                        text <=> search_text;
                        edited(text) => { search_changed(text); }
                    }
                }
            }

            // Tree view header
            Rectangle {
                height: 24px;
                background: #333333;
                border-width: 1px;
                border-color: #444444;

                HorizontalLayout {
                    padding-left: 8px;
                    Text {
                        text: "File";
                        font-weight: 600;
                        vertical-alignment: center;
                        font-size: 12px;
                        color: #e0e0e0;
                    }
                }
            }

            // Tree view content
            Rectangle {
                vertical-stretch: 1;
                background: #252525;
                border-width: 1px;
                border-color: #444444;
                clip: true;

                ListView {
                    for node[idx] in tree_nodes: TreeRow {
                        node: node;
                        odd_row: mod(idx, 2) == 1;
                        toggle_expand(i) => { root.toggle_expand(i); }
                        toggle_select(i) => { root.toggle_select(i); }
                    }
                }

                if tree_nodes.length == 0: Text {
                    text: "Drag and drop BSA/BA2 file here\nor use File → Open";
                    horizontal-alignment: center;
                    vertical-alignment: center;
                    color: #666666;
                    font-size: 14px;
                }
            }

            // Progress bar (only shown when processing)
            if is_processing: Rectangle {
                height: 20px;
                background: #333333;

                Rectangle {
                    x: 0;
                    y: 0;
                    width: parent.width * progress;
                    height: parent.height;
                    background: #4a9eff;
                }

                Text {
                    text: round(progress * 100) + "%";
                    horizontal-alignment: center;
                    vertical-alignment: center;
                    font-size: 11px;
                    color: #ffffff;
                }
            }

            // Status bar
            Rectangle {
                height: 24px;
                background: #2d2d2d;
                border-width: 1px;
                border-color: #444444;

                HorizontalLayout {
                    padding-left: 8px;
                    Text {
                        text: status_text;
                        vertical-alignment: center;
                        font-size: 11px;
                        color: #aaaaaa;
                        overflow: elide;
                    }
                }
            }

            // Action button (Extract or Pack depending on mode)
            Rectangle {
                height: 40px;
                background: #2d2d2d;

                HorizontalLayout {
                    padding: 4px;

                    if !pack_mode: Button {
                        text: "Extract";
                        horizontal-stretch: 1;
                        enabled: tree_nodes.length > 0 && !is_processing;
                        clicked => { extract(); }
                    }

                    if pack_mode: Button {
                        text: "Pack";
                        horizontal-stretch: 1;
                        enabled: tree_nodes.length > 0 && !is_processing;
                        clicked => { pack(); }
                    }
                }
            }
        }

        // File menu popup
        file_popup := PopupWindow {
            x: 4px;
            y: 28px;
            width: 150px;
            height: 70px;

            Rectangle {
                background: #2d2d2d;
                border-width: 1px;
                border-color: #444444;
                drop-shadow-blur: 4px;
                drop-shadow-color: #00000080;

                VerticalLayout {
                    padding: 2px;

                    Rectangle {
                        height: 24px;
                        background: open_file_touch.has-hover ? #3d5a80 : transparent;

                        open_file_touch := TouchArea {
                            clicked => {
                                file_popup.close();
                                open_file();
                            }
                        }

                        HorizontalLayout {
                            padding-left: 8px;
                            Text {
                                text: "Open Archive...";
                                vertical-alignment: center;
                                font-size: 12px;
                                color: #e0e0e0;
                            }
                        }
                    }

                    Rectangle {
                        height: 24px;
                        background: open_folder_touch.has-hover ? #3d5a80 : transparent;

                        open_folder_touch := TouchArea {
                            clicked => {
                                file_popup.close();
                                open_folder();
                            }
                        }

                        HorizontalLayout {
                            padding-left: 8px;
                            Text {
                                text: "Open Folder...";
                                vertical-alignment: center;
                                font-size: 12px;
                                color: #e0e0e0;
                            }
                        }
                    }
                }
            }
        }

        // About popup
        about_popup := PopupWindow {
            x: (root.width - 300px) / 2;
            y: (root.height - 150px) / 2;
            width: 300px;
            height: 150px;

            Rectangle {
                background: #2d2d2d;
                border-width: 1px;
                border-color: #444444;
                drop-shadow-blur: 8px;
                drop-shadow-color: #000000a0;

                VerticalLayout {
                    padding: 16px;
                    spacing: 8px;
                    alignment: center;

                    Text {
                        text: "BSA/BA2 Archive Tool";
                        font-size: 16px;
                        font-weight: 700;
                        horizontal-alignment: center;
                        color: #ffffff;
                    }

                    Text {
                        text: "Version 0.1.0";
                        font-size: 12px;
                        horizontal-alignment: center;
                        color: #aaaaaa;
                    }

                    Text {
                        text: "Supports TES3, TES4, and BA2 archives";
                        font-size: 11px;
                        horizontal-alignment: center;
                        color: #888888;
                    }

                    Rectangle { height: 8px; }

                    Button {
                        text: "OK";
                        clicked => { about_popup.close(); }
                    }
                }
            }
        }
    }
}
