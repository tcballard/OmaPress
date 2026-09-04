import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import QtQuick.Dialogs

ApplicationWindow {
    id: win
    width: 1440; height: 930; minimumWidth: 900; minimumHeight: 620
    visible: true
    title: (dirty ? "• " : "") + (meta.title || "OmaPress") + (publication.config ? " — " + publication.config.name : "")
    color: backend.background
    Material.theme: Qt.darker(backend.background, 1).r + Qt.darker(backend.background, 1).g + Qt.darker(backend.background, 1).b > 1.5 ? Material.Light : Material.Dark
    Material.accent: backend.accent
    Material.primary: backend.background
    Material.background: backend.background
    Material.foreground: backend.foreground
    font.family: "Sans Serif"; font.pixelSize: 14

    property var publication: ({})
    property var meta: ({})
    property string articlePath: ""
    property string sourceHash: ""
    property bool dirty: false
    property int editRevision: 0
    property int savingRevision: -1
    property bool loading: false
    property bool closingAllowed: false
    property bool showPreview: true
    property string filter: "all"
    property string query: ""
    property string notice: ""
    property bool noticeError: false
    property string rendered: ""
    property var exportData: ({html:"",text:"",caption:"",warnings:[]})
    property var publishPlan: ({})
    property var recovery: ({})
    property var pendingAction: null
    property string rollbackId: ""
    property var repoChoices: []
    readonly property bool opened: !!publication.config
    readonly property bool hasArticle: articlePath !== ""
    readonly property var seriesIds: opened ? Object.keys(publication.config.series) : []
    readonly property var shownArticles: (publication.articles || []).filter(function(a) {
        return (filter === "all" || a.display_status === filter) && (!query || a.search.toLowerCase().indexOf(query.toLowerCase()) >= 0 || a.meta.series.indexOf(query.toLowerCase()) >= 0 || (a.meta.published_at || "").indexOf(query) >= 0)
    })

    function say(text, error) { notice = text; noticeError = !!error }
    function guarded(action) { if (dirty) { pendingAction = action; unsavedDialog.open() } else action() }
    function openPublication(path) { backend.publicationPath = path; backend.request("inspect", {}, "open") }
    function chooseArticle(path) { guarded(function(){ backend.request("read", {article:path}, "read") }) }
    function changed() { if (loading || !hasArticle) return; dirty = true; editRevision++; renderTimer.restart(); recoveryTimer.restart() }
    function setMeta(key, value) { if (loading) return; var next = Object.assign({},meta); next[key] = value; meta = next; changed() }
    function docArgs() { return {article:articlePath,meta:meta,body:editor.text,expected_source_hash:sourceHash} }
    function save() { if (hasArticle) { savingRevision = editRevision; backend.request("save-document", docArgs(), "save") } }
    function renderNow() { if (hasArticle) backend.request("render-document", docArgs(), "render") }
    function requestPublish() { guarded(function(){ rollbackId = ""; backend.request("publish-plan",{expected_source_hash:sourceHash},"plan") }) }
    function setDocument(value) {
        loading = true; articlePath = value.article.path; meta = value.article.meta; editor.text = value.article.body;
        sourceHash = value.source_hash; dirty = false; loading = false; renderNow();
        if (value.recovery && (value.recovery.body !== editor.text || JSON.stringify(value.recovery.meta) !== JSON.stringify(meta))) { recovery = value.recovery; recoveryDialog.open() }
    }
    function newArticle() { guarded(function(){backend.request("new",{series:seriesIds[0],expected_source_hash:sourceHash},"new")}) }
    function insertMarkup(before, after) { var start = editor.selectionStart; var text = editor.selectedText; editor.remove(start,editor.selectionEnd); editor.insert(start,before+text+after); editor.cursorPosition=start+before.length+text.length; editor.forceActiveFocus() }
    function findNext() { var from = editor.selectionEnd; var n = editor.text.toLowerCase().indexOf(findField.text.toLowerCase(),from); if(n<0)n=editor.text.toLowerCase().indexOf(findField.text.toLowerCase()); if(n>=0){editor.select(n,n+findField.text.length);editor.forceActiveFocus()} }

    Component.onCompleted: { var last = backend.lastPublication(); if (last) openPublication(last) }
    onClosing: function(close) { if(dirty && !closingAllowed){close.accepted=false;pendingAction=function(){closingAllowed=true;win.close()};unsavedDialog.open()} }
    Timer { id: renderTimer; interval: 300; onTriggered: renderNow() }
    Timer { id: recoveryTimer; interval: 1000; onTriggered: { if(dirty)backend.request("recovery-document",docArgs(),"recover-save") } }
    Shortcut { sequence: StandardKey.Save; enabled: hasArticle; onActivated: save() }
    Shortcut { sequence: StandardKey.New; enabled: opened; onActivated: newArticle() }
    Shortcut { sequence: StandardKey.Open; onActivated: guarded(function(){openFolder.open()}) }
    Shortcut { sequence: StandardKey.Find; enabled: hasArticle; onActivated: {findBar.visible=true;findField.forceActiveFocus()} }
    Shortcut { sequence: "Ctrl+Shift+P"; enabled: opened; onActivated: requestPublish() }
    Shortcut { sequence: "Ctrl+Shift+V"; enabled: editor.activeFocus; onActivated: editor.paste() }
    Shortcut { sequence: "Ctrl+B"; enabled: editor.activeFocus; onActivated: insertMarkup("**","**") }
    Shortcut { sequence: "Ctrl+I"; enabled: editor.activeFocus; onActivated: insertMarkup("*","*") }

    Connections {
        target: backend
        function onFailed(tag,error) { say(error,true) }
        function onArrayResult(tag,value) { if(tag==="repos")repoChoices=value }
        function onResult(tag,v) {
            if(tag==="open"||tag==="init") { publication=v;sourceHash=v.source_hash;articlePath="";meta={};editor.text="";dirty=false;if((v.articles||[]).length)backend.request("read",{article:v.articles[0].path},"read") }
            else if(tag==="refresh") {publication=v;if(!dirty)sourceHash=v.source_hash}
            else if(tag==="read") setDocument(v)
            else if(tag==="new"||tag==="import") {publication=v.publication;sourceHash=v.publication.source_hash;backend.request("read",{article:v.article},"read")}
            else if(tag==="save") {publication=v;sourceHash=v.source_hash;dirty=(editRevision!==savingRevision);if(!dirty){recoveryTimer.stop();backend.request("recovery-clear",{article:articlePath},"cleared")}else recoveryTimer.restart();say(dirty?"Saved. Newer edits remain unsaved.":"Saved locally.");if(pendingAction&&!dirty){var next=pendingAction;pendingAction=null;next()}}
            else if(tag==="render") {rendered=v.html;exportData=v.export}
            else if(tag==="validate") {publication=Object.assign({},publication,{diagnostics:v.diagnostics});checksDialog.open()}
            else if(tag==="build") {say("Site and feeds built. Preview the exact site before publishing.");backend.startPreview(false)}
            else if(tag==="media") {sourceHash=v.source_hash;setMeta("header_image",v.media_path);say("Image imported into this publication.")}
            else if(tag==="settings-save") {publication=v;sourceHash=v.source_hash;settingsDialog.close();say("Publication settings saved.")}
            else if(tag==="plan") {publishPlan=v;publishDialog.open()}
            else if(tag==="publish"||tag==="recheck"||tag==="rollback") {say(v.status==="published"?"Publication verified and live.":(v.message||"Deployment needs a recheck."),v.status!=="published");backend.request("inspect",{},"refresh")}
            else if(tag==="github")say("Signed in to GitHub as "+v.login+".")
            else if(tag==="setup")say(v.message)
            else if(tag==="delete") {publication=v;sourceHash=v.source_hash;articlePath="";meta={};editor.text="";dirty=false}
        }
    }

    header: ToolBar {
        padding: 8
        RowLayout {
            anchors.fill: parent; spacing: 12
            Label { text:"OmaPress";font.pixelSize:20;font.bold:true;Layout.leftMargin:8 }
            Label { text:opened?publication.config.name:"Your words. Your publication.";elide:Text.ElideRight;Layout.fillWidth:true;opacity:.65 }
            BusyIndicator { running:backend.busy;implicitWidth:24;implicitHeight:24;Accessible.name:"Operation in progress" }
            Button { text:"Save";enabled:hasArticle&&dirty;onClicked:save();Accessible.name:"Save article locally" }
            Button { text:"Preview site";enabled:opened;onClicked:guarded(function(){backend.startPreview(true)});ToolTip.text:"Open a private local preview, including drafts";ToolTip.visible:hovered }
            Button { text:"Publish…";highlighted:true;enabled:opened&&!backend.busy;onClicked:requestPublish() }
        }
    }
    ColumnLayout {
        anchors.fill:parent;spacing:0
        Rectangle {
            visible:notice!=="";Layout.fillWidth:true;implicitHeight:noticeLabel.implicitHeight+20;color:noticeError?"#572d2d":Qt.lighter(backend.background,1.45)
            RowLayout { anchors.fill:parent;anchors.margins:10
                Label { id:noticeLabel;text:notice;wrapMode:Text.Wrap;Layout.fillWidth:true;color:noticeError?"#fff0ee":backend.foreground;Accessible.role:Accessible.AlertMessage }
                ToolButton { text:"×";Accessible.name:"Dismiss notification";onClicked:notice="" }
            }
        }
        RowLayout {
            Layout.fillWidth:true;Layout.fillHeight:true;spacing:0
            Pane {
                Layout.preferredWidth:210;Layout.fillHeight:true;padding:14
                ColumnLayout { anchors.fill:parent;spacing:8
                    Label { text:"PUBLICATION";font.pixelSize:11;font.letterSpacing:1.5;opacity:.5;Layout.topMargin:10;Layout.bottomMargin:12 }
                    Button { text:"Open…";Layout.fillWidth:true;onClicked:guarded(function(){openFolder.open()}) }
                    Button { text:"Create…";Layout.fillWidth:true;onClicked:guarded(function(){createDialog.open()}) }
                    Item { implicitHeight:16 }
                    Repeater { model:[{key:"all",label:"All articles"},{key:"draft",label:"Drafts"},{key:"ready",label:"Ready"},{key:"published",label:"Published"}]
                        delegate:Button { required property var modelData;text:modelData.label;flat:true;highlighted:filter===modelData.key;Layout.fillWidth:true;onClicked:filter=modelData.key }
                    }
                    Item { Layout.fillHeight:true }
                    Label { text:opened&&publication.state&&publication.state.pending?"Deployment needs recheck":opened&&publication.state&&publication.state.deployments.length?"Last deployment verified":"Local publication";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.65;font.pixelSize:12 }
                    Button { text:"Deployment history";enabled:opened;flat:true;Layout.fillWidth:true;onClicked:historyDialog.open() }
                    Button { text:"Settings";enabled:opened;flat:true;Layout.fillWidth:true;onClicked:guarded(function(){configEditor.text=publication.config_text;settingsDialog.open()}) }
                    Label { text:"OmaPress 0.1.0-rc.1";font.pixelSize:11;opacity:.4 }
                }
            }
            Rectangle { Layout.fillHeight:true;implicitWidth:1;color:backend.foreground;opacity:.13 }
            ColumnLayout {
                visible:opened;Layout.preferredWidth:260;Layout.maximumWidth:290;Layout.fillHeight:true;spacing:0
                RowLayout { Layout.fillWidth:true;Layout.margins:12
                    TextField { placeholderText:"Search articles…";Layout.fillWidth:true;onTextChanged:query=text;Accessible.name:"Search title, text, tags, series and date" }
                    ToolButton { text:"+";Accessible.name:"New article";onClicked:newArticle() }
                }
                ListView {
                    id:articleList;Layout.fillWidth:true;Layout.fillHeight:true;clip:true;model:shownArticles;spacing:1
                    delegate:ItemDelegate {
                        required property var modelData;width:articleList.width;height:118;highlighted:articlePath===modelData.path
                        contentItem:ColumnLayout {spacing:5
                            Label {text:modelData.display_status.toUpperCase()+" · "+(modelData.meta.published_at||"").slice(0,10);font.pixelSize:10;font.letterSpacing:.7;color:backend.accent}
                            Label {text:modelData.meta.title||"Untitled article";font.bold:true;wrapMode:Text.Wrap;maximumLineCount:2;elide:Text.ElideRight;Layout.fillWidth:true}
                            Label {text:publication.config.series[modelData.meta.series]?publication.config.series[modelData.meta.series].title:modelData.meta.series;font.pixelSize:11;opacity:.55;elide:Text.ElideRight;Layout.fillWidth:true}
                        }
                        onClicked:chooseArticle(modelData.path)
                    }
                    Label {anchors.centerIn:parent;visible:articleList.count===0;text:"No articles here yet";opacity:.5}
                    ScrollBar.vertical:ScrollBar {}
                }
                Button {text:"Import Markdown…";Layout.fillWidth:true;Layout.margins:10;onClicked:guarded(function(){articleImport.open()})}
            }
            Rectangle {visible:opened;Layout.fillHeight:true;implicitWidth:1;color:backend.foreground;opacity:.13}
            ColumnLayout {
                visible:hasArticle;Layout.fillWidth:true;Layout.fillHeight:true;spacing:0
                RowLayout {Layout.fillWidth:true;Layout.margins:10
                    Label {text:dirty?"Unsaved changes":"Saved locally";font.pixelSize:12;opacity:.65;Layout.fillWidth:true}
                    ToolButton {text:"Metadata";onClicked:metadataDialog.open()}
                    ToolButton {text:"Checks";onClicked:guarded(function(){backend.request("validate",{},"validate")})}
                    ToolButton {text:"X export";onClicked:{renderNow();exportDialog.open()}}
                    ToolButton {text:showPreview?"Hide preview":"Show preview";onClicked:showPreview=!showPreview}
                }
                TextField {text:meta.title||"";placeholderText:meta.title?"":"Your editorial title";font.pixelSize:27;font.bold:true;Layout.fillWidth:true;Layout.leftMargin:24;Layout.rightMargin:24;background:Item{} onTextEdited:setMeta("title",text);Accessible.name:"Editorial title"}
                TextField {text:meta.summary||"";placeholderText:meta.summary?"":"A short summary for the site and feeds";Layout.fillWidth:true;Layout.leftMargin:24;Layout.rightMargin:24;background:Item{} onTextEdited:setMeta("summary",text);Accessible.name:"Article summary"}
                RowLayout {Layout.leftMargin:18;Layout.rightMargin:18
                    ToolButton {text:"H2";Accessible.name:"Insert heading";onClicked:insertMarkup("\n## ","\n")}
                    ToolButton {text:"B";font.bold:true;Accessible.name:"Bold";onClicked:insertMarkup("**","**")}
                    ToolButton {text:"I";font.italic:true;Accessible.name:"Italic";onClicked:insertMarkup("*","*")}
                    ToolButton {text:"Link";onClicked:insertMarkup("[","](https://)")}
                    ToolButton {text:"List";onClicked:insertMarkup("\n- ","\n")}
                    ToolButton {text:"Code";onClicked:insertMarkup("\n```\n","\n```\n")}
                    Item {Layout.fillWidth:true}
                    ToolButton {text:"Undo";enabled:editor.canUndo;onClicked:editor.undo()}
                    ToolButton {text:"Redo";enabled:editor.canRedo;onClicked:editor.redo()}
                }
                RowLayout {id:findBar;visible:false;Layout.fillWidth:true;Layout.margins:12
                    TextField {id:findField;placeholderText:"Find in article";Layout.fillWidth:true;onAccepted:findNext();Accessible.name:"Find in article"}
                    Button {text:"Next";onClicked:findNext()}
                    ToolButton {text:"×";Accessible.name:"Close find";onClicked:findBar.visible=false}
                }
                SplitView {
                    Layout.fillWidth:true;Layout.fillHeight:true;orientation:Qt.Horizontal
                    ScrollView {
                        SplitView.fillWidth:true;SplitView.minimumWidth:250;clip:true
                        TextArea {id:editor;textFormat:TextEdit.PlainText;wrapMode:TextEdit.Wrap;selectByMouse:true;font.family:"monospace";font.pixelSize:15;leftPadding:24;rightPadding:24;topPadding:20;bottomPadding:80;placeholderText:editor.text?"":"Start writing in Markdown…";onTextChanged:changed();Accessible.name:"Markdown article body"}
                    }
                    ScrollView {
                        visible:showPreview&&win.width>=1150;SplitView.preferredWidth:Math.max(260,win.width*0.25);SplitView.minimumWidth:220;clip:true
                        TextArea {text:rendered;textFormat:TextEdit.RichText;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;font.family:"serif";font.pixelSize:18;leftPadding:24;rightPadding:24;topPadding:20;bottomPadding:80;onLinkActivated:function(link){backend.openUrl(link)} Accessible.name:"Rendered article preview"}
                    }
                }
                Label {text:(editor.text.trim()?editor.text.trim().split(/\s+/).length:0)+" words · Markdown · "+(meta.series||"");font.pixelSize:11;opacity:.5;Layout.margins:12}
            }
            Item {visible:!hasArticle;Layout.fillWidth:true;Layout.fillHeight:true
                ColumnLayout {anchors.centerIn:parent;width:Math.min(parent.width-64,480);spacing:18
                    Label {text:opened?"Make room for your next story.":"A home for your publication.";font.pixelSize:32;font.bold:true;wrapMode:Text.Wrap;Layout.fillWidth:true}
                    Label {text:opened?"Write locally, review the site, then publish. Your articles and feeds stay yours.":"Keep your articles in a folder you own. Build a website and feeds, then take your words to X.";font.pixelSize:17;wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.6}
                    Button {text:opened?"Write an article":"Create a publication";highlighted:true;onClicked:opened?newArticle():createDialog.open()}
                }
            }
        }
    }

    FolderDialog {id:openFolder;title:"Open publication folder";onAccepted:openPublication(backend.localPath(selectedFolder))}
    FolderDialog {id:createParent;title:"Choose where to create the publication";onAccepted:createPath.text=backend.localPath(selectedFolder)+"/fixing-everything"}
    FileDialog {id:mediaImport;title:"Import header image";nameFilters:["Images (*.png *.jpg *.jpeg *.webp *.gif *.avif)"];onAccepted:backend.request("import-media",{source:backend.localPath(selectedFile),expected_source_hash:sourceHash},"media")}
    FileDialog {id:articleImport;title:"Import Markdown article with OmaPress front matter";nameFilters:["Markdown (*.md)"];onAccepted:backend.request("import-article",{source:backend.localPath(selectedFile),expected_source_hash:sourceHash},"import")}

    Dialog {id:createDialog;title:"Create a publication";modal:true;anchors.centerIn:parent;width:540;standardButtons:Dialog.Cancel
        ColumnLayout {width:parent.width;spacing:12
            Label {text:"A new folder holds your articles, media and settings.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            TextField {id:createName;text:"My publication";placeholderText:"Publication name";Layout.fillWidth:true;Accessible.name:"Publication name"}
            TextField {id:createAuthor;placeholderText:"Your name";Layout.fillWidth:true;Accessible.name:"Author"}
            TextField {id:createUrl;text:"https://example.invalid";placeholderText:"Canonical URL";Layout.fillWidth:true;Accessible.name:"Canonical publication URL"}
            Label {text:"Use your confirmed domain or GitHub Pages URL. You can change it before the first publish.";wrapMode:Text.Wrap;Layout.fillWidth:true;font.pixelSize:12;opacity:.65}
            RowLayout {Layout.fillWidth:true;TextField{id:createPath;placeholderText:"New publication folder";Layout.fillWidth:true;Accessible.name:"Publication folder"}Button{text:"Choose…";onClicked:createParent.open()}}
            Button {text:"Create publication";highlighted:true;enabled:createPath.text!=="";onClicked:{backend.publicationPath=createPath.text;backend.request("init",{name:createName.text,author:createAuthor.text,base_url:createUrl.text},"init");createDialog.close()}}
        }
    }
    Dialog {id:metadataDialog;title:"Article details";modal:true;anchors.centerIn:parent;width:600;height:Math.min(win.height-70,740);standardButtons:Dialog.Close
        ScrollView {anchors.fill:parent;clip:true
            ColumnLayout {width:metadataDialog.availableWidth-20;spacing:10
                Label {text:"Series"}
                ComboBox {model:seriesIds;textRole:"";currentIndex:Math.max(0,seriesIds.indexOf(meta.series));Layout.fillWidth:true;onActivated:setMeta("series",currentText);Accessible.name:"Article series"}
                Label {text:"Editorial status"}
                ComboBox {model:["draft","ready"];currentIndex:meta.status==="draft"?0:1;onActivated:setMeta("status",currentText);Accessible.name:"Draft or ready"}
                Label {text:"Publication date (including timezone)"}
                TextField {text:meta.published_at||"";Layout.fillWidth:true;onTextEdited:setMeta("published_at",text);Accessible.name:"Publication date"}
                Label {text:"Permanent slug"}
                TextField {text:meta.slug||"";Layout.fillWidth:true;onTextEdited:setMeta("slug",text);Accessible.name:"Article slug"}
                Label {text:"Tags, separated by commas"}
                TextField {text:(meta.tags||[]).join(", ");Layout.fillWidth:true;onTextEdited:setMeta("tags",text.split(",").map(function(t){return t.trim()}).filter(function(t){return t.length>0}));Accessible.name:"Article tags"}
                RowLayout {Layout.fillWidth:true;Label{text:"Header image";Layout.fillWidth:true}Button{text:"Import image…";onClicked:mediaImport.open()}}
                TextField {text:meta.header_image||"";Layout.fillWidth:true;onTextEdited:setMeta("header_image",text||null);Accessible.name:"Header image path"}
                Label {text:"X caption"}
                TextArea {text:meta.x_caption||"";Layout.fillWidth:true;wrapMode:TextEdit.Wrap;implicitHeight:110;onTextChanged:{if(activeFocus)setMeta("x_caption",text)} Accessible.name:"X caption"}
                Label {text:"Published X Article URL (optional)"}
                TextField {text:meta.x_url||"";Layout.fillWidth:true;onTextEdited:setMeta("x_url",text||null);Accessible.name:"Published X Article URL"}
                Button {text:"Delete unpublished draft…";enabled:meta.status==="draft";onClicked:deleteDialog.open()}
            }
        }
    }
    Dialog {id:unsavedDialog;title:"Save your changes?";modal:true;anchors.centerIn:parent;width:460
        ColumnLayout {width:parent.width
            Label {text:"This article has changes that have not been saved.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            RowLayout {Button{text:"Cancel";onClicked:{pendingAction=null;unsavedDialog.close()}}Button{text:"Discard";onClicked:{dirty=false;recoveryTimer.stop();backend.request("recovery-clear",{article:articlePath},"cleared");var next=pendingAction;pendingAction=null;unsavedDialog.close();if(next)next()}}Button{text:"Save";highlighted:true;onClicked:{unsavedDialog.close();save()}}}
        }
    }
    Dialog {id:recoveryDialog;title:"Recover unsaved work?";modal:true;anchors.centerIn:parent;width:480
        ColumnLayout {width:parent.width
            Label {text:"A recovery copy was saved at "+(recovery.saved_at||"")+". Restore it to the editor and review it before saving.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            RowLayout {Button{text:"Keep saved article";onClicked:{backend.request("recovery-clear",{article:articlePath},"cleared");recoveryDialog.close()}}Button{text:"Restore recovery";highlighted:true;onClicked:{loading=true;meta=recovery.meta||meta;editor.text=recovery.body||"";sourceHash=recovery.base_source_hash||sourceHash;loading=false;dirty=true;renderNow();recoveryDialog.close()}}}
        }
    }
    Dialog {id:checksDialog;title:"Publication checks";modal:true;anchors.centerIn:parent;width:650;height:Math.min(win.height-90,560);standardButtons:Dialog.Close
        ScrollView {anchors.fill:parent;clip:true;ColumnLayout {width:checksDialog.availableWidth-20;spacing:12
            Label {visible:(publication.diagnostics||[]).length===0;text:"All checks passed.";color:backend.accent}
            Repeater {model:publication.diagnostics||[];delegate:Label {required property var modelData;text:modelData.severity.toUpperCase()+" · "+modelData.path+"\n"+modelData.message;wrapMode:Text.Wrap;Layout.fillWidth:true}}
            Button {text:"Build site and validate feeds";onClicked:{checksDialog.close();backend.request("build",{expected_source_hash:sourceHash},"build")}}
        }}
    }
    Dialog {id:exportDialog;title:"Copy to X Articles";modal:true;anchors.centerIn:parent;width:750;height:Math.min(win.height-70,780);standardButtons:Dialog.Close
        ColumnLayout {anchors.fill:parent;spacing:12
            Label {text:"Review this version, then paste it into X yourself.";opacity:.7}
            Repeater {model:exportData.warnings||[];delegate:Label {required property string modelData;text:modelData;wrapMode:Text.Wrap;Layout.fillWidth:true;color:backend.accent}}
            TabBar {id:exportTabs;Layout.fillWidth:true;TabButton{text:"Article"}TabButton{text:"Plain text"}TabButton{text:"Caption"}}
            StackLayout {currentIndex:exportTabs.currentIndex;Layout.fillWidth:true;Layout.fillHeight:true
                ScrollView {clip:true;TextArea{text:exportData.preview_html||"";textFormat:TextEdit.RichText;readOnly:true;wrapMode:TextEdit.Wrap;Accessible.name:"X rich article preview"}}
                ScrollView {clip:true;TextArea{text:exportData.text||"";readOnly:true;wrapMode:TextEdit.Wrap;Accessible.name:"X plain text preview"}}
                ScrollView {clip:true;TextArea{text:exportData.caption||"";readOnly:true;wrapMode:TextEdit.Wrap;Accessible.name:"X caption preview"}}
            }
            RowLayout {Button{text:"Copy article";highlighted:true;enabled:!backend.busy;onClicked:{backend.copyArticle(exportData.html,exportData.text);say("Article copied as HTML and plain text.")}}Button{text:"Copy caption";enabled:!backend.busy;onClicked:{backend.copyText(exportData.caption);say("Caption copied.")}}}
        }
    }
    Dialog {id:settingsDialog;title:"Publication settings";modal:true;anchors.centerIn:parent;width:760;height:Math.min(win.height-60,790);standardButtons:Dialog.Close
        ColumnLayout {anchors.fill:parent
            Label {text:"Publication details, series and appearance";font.bold:true}
            ScrollView {Layout.fillWidth:true;Layout.fillHeight:true;clip:true;TextArea{id:configEditor;font.family:"monospace";wrapMode:TextEdit.NoWrap;Accessible.name:"Publication TOML settings"}}
            RowLayout {Button{text:"Save settings";highlighted:true;onClicked:backend.request("save-config",{text:configEditor.text,expected_source_hash:sourceHash},"settings-save")}Button{text:"Check GitHub login";onClicked:backend.request("github-status",{},"github")}Button{text:"Choose repository…";onClicked:{backend.request("repositories",{},"repos");repoDialog.open()}}}
        }
    }
    Dialog {id:repoDialog;title:"GitHub Pages destination";modal:true;anchors.centerIn:parent;width:580;standardButtons:Dialog.Close
        ColumnLayout {width:parent.width
            Label {text:"Select an existing repository or enter a name to create a public site repository.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            ComboBox {model:repoChoices;textRole:"nameWithOwner";Layout.fillWidth:true;onActivated:repoName.text=currentText;Accessible.name:"Existing GitHub repositories"}
            TextField {id:repoName;placeholderText:"owner/publication-site";Layout.fillWidth:true;Accessible.name:"Destination repository"}
            RowLayout {Button{text:"Check repository";onClicked:backend.request("setup",{repository:repoName.text,create:false,expected_source_hash:sourceHash},"setup")}Button{text:"Create public repository";onClicked:backend.request("setup",{repository:repoName.text,create:true,expected_source_hash:sourceHash},"setup")}}
            Label {text:"After checking, set deploy.repository in publication settings to this name.";wrapMode:Text.Wrap;Layout.fillWidth:true;font.pixelSize:12;opacity:.65}
        }
    }
    Dialog {id:publishDialog;title:rollbackId?"Review rollback":"Review publication";modal:true;anchors.centerIn:parent;width:630;standardButtons:Dialog.Cancel
        ColumnLayout {width:parent.width;spacing:14
            Label {text:rollbackId?"Republish a previous verified version. Article sources remain saved locally.":"This will update the public website and feeds.";wrapMode:Text.Wrap;Layout.fillWidth:true;font.pixelSize:18}
            Label {text:"Repository: "+(publishPlan.repository||"")+"\nBranch: "+(publishPlan.branch||"")+"\nCurrent remote: "+(publishPlan.expected_remote_head||"New branch")+"\nWebsite: "+(publishPlan.base_url||"")+"\nArticles included: "+(publishPlan.article_ids||[]).length;wrapMode:Text.WrapAnywhere;Layout.fillWidth:true}
            Label {text:"Reviewed source: "+(publishPlan.source_hash||"");font.family:"monospace";font.pixelSize:11;wrapMode:Text.WrapAnywhere;Layout.fillWidth:true;opacity:.6}
            Button {text:rollbackId?"Publish rollback":"Publish reviewed version";highlighted:true;onClicked:{var args={expected_source_hash:publishPlan.source_hash,expected_remote_head:publishPlan.expected_remote_head};if(rollbackId){args.deployment_id=rollbackId;backend.request("rollback",args,"rollback")}else backend.request("publish",args,"publish");publishDialog.close()}}
        }
    }
    Dialog {id:historyDialog;title:"Deployment history";modal:true;anchors.centerIn:parent;width:680;height:Math.min(win.height-70,600);standardButtons:Dialog.Close
        ScrollView {anchors.fill:parent;clip:true;ColumnLayout {width:historyDialog.availableWidth-20;spacing:12
            Button {text:"Recheck pending deployment";enabled:!!(publication.state&&publication.state.pending);onClicked:{historyDialog.close();backend.request("recheck",{},"recheck")}}
            Repeater {model:publication.state?publication.state.deployments.slice().reverse():[];delegate:Frame {required property var modelData;Layout.fillWidth:true;ColumnLayout {width:parent.width
                Label {text:modelData.verified_at;Layout.fillWidth:true}
                Label {text:modelData.remote_commit;font.family:"monospace";font.pixelSize:11;wrapMode:Text.WrapAnywhere;Layout.fillWidth:true;opacity:.6}
                Button {text:"Review rollback to this version…";onClicked:{var id=modelData.id;historyDialog.close();guarded(function(){rollbackId=id;backend.request("publish-plan",{expected_source_hash:sourceHash},"plan")})}}
            }}}
        }}
    }
    Dialog {id:deleteDialog;title:"Delete this draft?";modal:true;anchors.centerIn:parent;standardButtons:Dialog.Cancel|Dialog.Ok
        Label {text:"The draft will be moved to local recovery storage."}
        onAccepted:{metadataDialog.close();backend.request("delete",{article:articlePath,expected_source_hash:sourceHash},"delete")}
    }
}
