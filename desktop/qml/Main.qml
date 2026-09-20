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
    property bool showPreview: false
    property string filter: "all"
    property string query: ""
    property string notice: ""
    property bool noticeError: false
    property string rendered: ""
    property var exportData: ({html:"",text:"",caption:"",warnings:[]})
    property var publishPlan: ({})
    property var gatewayReceipt: ({})
    property var gatewayConnection: ({})
    property var xConnection: ({})
    property var distribution: ({})
    property var distributionReview: ({})
    property var recovery: ({})
    property var pendingAction: null
    property string rollbackId: ""
    property var queueState: ({jobs:[]})
    property string queueId: ""
    property string remoteSource: ""
    property var repoChoices: []
    function queueRequest(command, args, tag) {
        if(remoteEnabled.checked) { backend.saveWorkerSettings(workerHost.text,workerPath.text);backend.request("remote-queue",{host:workerHost.text,path:workerPath.text,command:command,args:args},tag) }
        else backend.request(command,args,tag)
    }
    function showQueue() { var settings=backend.workerSettings();workerHost.text=settings.host||"";workerPath.text=settings.path||"";queueDialog.open(); queueRequest("queue-list",{},"queue-list") }
    readonly property color surface: Qt.lighter(backend.background, 1.22)
    readonly property color border: Qt.rgba(backend.foreground.r, backend.foreground.g, backend.foreground.b, 0.10)

    component WorkspaceButton: Button {
        id: control
        font.capitalization: Font.MixedCase
        implicitHeight: 36
        topInset: 0; bottomInset: 0
        leftPadding: 14; rightPadding: 14
        background: Rectangle {
            radius: 6
            color: control.highlighted ? backend.accent : control.down ? Qt.lighter(win.surface, 1.4) : control.hovered ? win.surface : "transparent"
            border.width: control.flat || control.highlighted ? 0 : 1
            border.color: win.border
            opacity: control.enabled ? 1 : 0.4
        }
        contentItem: Text {
            text: control.text; font: control.font
            color: control.highlighted ? backend.background : backend.foreground
            opacity: control.enabled ? 1 : 0.35
            horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
    }
    function smokeQueue() { intakeDialog.open(); queueDialog.open(); gatewayConfigDialog.open() }

    readonly property bool opened: !!publication.config
    readonly property bool hasArticle: articlePath !== ""
    readonly property var seriesIds: opened ? Object.keys(publication.config.series) : []
    readonly property var shownArticles: (publication.articles || []).filter(function(a) {
        return (filter === "all" || a.display_status === filter) && (!query || a.search.toLowerCase().indexOf(query.toLowerCase()) >= 0 || a.meta.series.indexOf(query.toLowerCase()) >= 0 || (a.meta.published_at || "").indexOf(query) >= 0)
    })

    function escapeHtml(text) { return String(text || "").replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;") }
    function say(text, error) { notice = text; noticeError = !!error }
    function guarded(action) { if (dirty) { pendingAction = action; unsavedDialog.open() } else action() }
    function openPublication(path) { backend.publicationPath = path; backend.request("inspect", {}, "open") }
    function chooseArticle(path) { guarded(function(){ backend.request("read", {article:path}, "read") }) }
    function changed() { if (loading || !hasArticle) return; dirty = true; editRevision++; renderTimer.restart(); recoveryTimer.restart() }
    function setMeta(key, value) { if (loading) return; var next = Object.assign({},meta); next[key] = value; meta = next; changed() }
    function docArgs() { return {article:articlePath,meta:meta,body:editor.text,expected_source_hash:sourceHash} }
    function save() { if (hasArticle) { savingRevision = editRevision; backend.request("save-document", docArgs(), "save") } }
    function renderNow() { if (hasArticle) backend.request("render-document", docArgs(), "render") }
    function requestPublish() { guarded(function(){
        if (hasArticle) backend.request("distribution-plan",{article:articlePath,expected_source_hash:sourceHash},"distribution")
        else { rollbackId="";backend.request("publish-plan",{expected_source_hash:sourceHash},"plan") }
    }) }
    function smokeConnections() { connectionsDialog.open() }
    function smokeDistribution() { connectionsDialog.close(); distributionDialog.open() }
    function distributionArgs() { return {article:distribution.article,expected_source_hash:distribution.source_hash} }
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
    Shortcut { sequence: StandardKey.Find; enabled: hasArticle; onActivated: {showPreview=false;findBar.visible=true;findField.forceActiveFocus()} }
    Shortcut { sequence: "Ctrl+Shift+P"; enabled: opened; onActivated: requestPublish() }
    Shortcut { sequence: "Ctrl+Shift+V"; enabled: editor.activeFocus; onActivated: editor.paste() }
    Shortcut { sequence: "Ctrl+B"; enabled: editor.activeFocus; onActivated: insertMarkup("**","**") }
    Shortcut { sequence: "Ctrl+I"; enabled: editor.activeFocus; onActivated: insertMarkup("*","*") }

    Connections {
        target: backend
        function onFailed(tag,error) { if(tag==="gateway-connect"){gatewaySession.text="";gatewayConnectSession.text=""}say(error,true) }
        function onArrayResult(tag,value) { if(tag==="repos")repoChoices=value }
        function onResult(tag,v) {
            if(tag==="open"||tag==="init") { publication=v;sourceHash=v.source_hash;articlePath="";meta={};editor.text="";dirty=false;if((v.articles||[]).length)backend.request("read",{article:v.articles[0].path},"read") }
            else if(tag==="refresh") {publication=v;if(!dirty)sourceHash=v.source_hash}
            else if(tag==="gateway-connect"||tag==="gateway-status") {gatewayConnection=v;gatewaySession.text="";gatewayConnectSession.text="";say(v.message||"Gateway configuration loaded.")}
            else if(tag==="gateway-action") {gatewayReceipt=v.receipt;gatewayReviewed.checked=false;say(v.message||"Substack scheduling receipt recorded.");backend.request("distribution-plan",distributionArgs(),"x-action")}
            else if(tag==="queue-list"||tag==="queue-added"||tag==="queue-cancelled"||tag==="queue-uploaded") {
                queueState=v;remoteSource=v.source_hash||"";
                if(tag==="queue-added") { queueId=""; say("Job accepted by the queue. Its worker must be running at the scheduled time.") }
                if(tag==="queue-uploaded") say("Reviewed source transferred. Configure the worker timer and credentials on that host before relying on scheduling.")
            }
            else if(tag==="read") {queueId="";gatewayReceipt={};gatewayReviewed.checked=false;setDocument(v)}
            else if(tag==="new"||tag==="import") {publication=v.publication;sourceHash=v.publication.source_hash;backend.request("read",{article:v.article},"read")}
            else if(tag==="save") {publication=v;sourceHash=v.source_hash;dirty=(editRevision!==savingRevision);if(!dirty){recoveryTimer.stop();backend.request("recovery-clear",{article:articlePath},"cleared")}else recoveryTimer.restart();say(dirty?"Saved. Newer edits remain unsaved.":"Saved locally.");if(pendingAction&&!dirty){var next=pendingAction;pendingAction=null;next()}}
            else if(tag==="render") {rendered=v.html;exportData=v.export}
            else if(tag==="validate") {publication=Object.assign({},publication,{diagnostics:v.diagnostics});checksDialog.open()}
            else if(tag==="build") {say("Site and feeds built. Preview the exact site before publishing.");backend.startPreview(false)}
            else if(tag==="media") {sourceHash=v.source_hash;setMeta("header_image",v.media_path);say("Image imported into this publication.")}
            else if(tag==="settings-save") {publication=v;sourceHash=v.source_hash;settingsDialog.close();say("Publication settings saved.")}
            else if(tag==="x-status"||tag==="x-connect"||tag==="x-disconnect") {xConnection=v;say(v.connected?"X account connected.":"X disconnected.")}
            else if(tag==="substack-prepare") {say(v.message);backend.request("distribution-plan",distributionArgs(),"x-action")}
            else if(tag==="distribution-review") {distributionReview=v;batchConfirm.open()}
            else if(tag==="distribution-publish") {distribution=v.distribution;var errors=v.results.filter(function(r){return !!r.error});say(errors.length?errors.map(function(r){return r.target+": "+r.error}).join("\n"):"Publishing finished. Check the recorded destination results.",errors.length>0);backend.request("inspect",{},"refresh")}
            else if(tag==="distribution") {distribution=v;var receipt=(v.targets||[]).filter(function(t){return t.target==="substack"})[0];gatewayReceipt=receipt&&receipt.receipt?receipt.receipt:{};gatewayReviewed.checked=false;distributionDialog.open()}
            else if(tag==="x-action"||tag==="confirm-destination") {distribution=v;say(tag==="x-action"?"Destination status refreshed.":"Published URL recorded as confirmed by you.")}
            else if(tag==="plan") {publishPlan=v;publishDialog.open()}
            else if(tag==="publish"||tag==="recheck"||tag==="rollback") {say(v.status==="published"?"Publication verified and live.":(v.message||"Deployment needs a recheck."),v.status!=="published");backend.request("inspect",{},"refresh");if(distributionDialog.visible)backend.request("distribution-plan",distributionArgs(),"x-action")}
            else if(tag==="github")say("Signed in to GitHub as "+v.login+".")
            else if(tag==="setup")say(v.message)
            else if(tag==="delete") {publication=v;sourceHash=v.source_hash;articlePath="";meta={};editor.text="";dirty=false}
        }
    }

    header: ToolBar {
        padding: 16
        background: Rectangle { color: backend.background
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: win.border }
        }
        RowLayout {
            anchors.fill: parent; spacing: 10
            Label { text:"OmaPress";font.pixelSize:21;font.bold:true;Layout.rightMargin:14 }
            Label { text:opened?publication.config.name:"Your words. Your publication.";elide:Text.ElideRight;Layout.fillWidth:true;opacity:.55 }
            BusyIndicator { running:backend.busy;implicitWidth:24;implicitHeight:24;Accessible.name:"Operation in progress" }
            WorkspaceButton { text:"Queue";flat:true;enabled:opened&&!backend.busy;onClicked:showQueue() }
            WorkspaceButton { text:"Connections";flat:true;onClicked:connectionsDialog.open() }
            WorkspaceButton { text:"Add article";enabled:opened&&!backend.busy;onClicked:guarded(function(){intakeDialog.open()}) }
            WorkspaceButton { text:"Publish…";highlighted:true;enabled:opened&&!backend.busy;onClicked:requestPublish() }
        }
    }
    ColumnLayout {
        anchors.fill:parent;spacing:0
        Rectangle {
            visible:notice!=="";Layout.fillWidth:true;implicitHeight:noticeLabel.implicitHeight+20;color:noticeError?"#572d2d":Qt.lighter(backend.background,1.45)
            RowLayout { anchors.fill:parent;anchors.margins:10
                Label { id:noticeLabel;text:notice;wrapMode:Text.Wrap;Layout.fillWidth:true;color:noticeError?"#fff0ee":backend.foreground;Accessible.role:Accessible.AlertMessage }
                ToolButton {font.capitalization:Font.MixedCase; text:"×";Accessible.name:"Dismiss notification";onClicked:notice="" }
            }
        }
        RowLayout {
            Layout.fillWidth:true;Layout.fillHeight:true;spacing:0
            Pane {
                Layout.preferredWidth:win.width>=1100?190:150;Layout.fillHeight:true;padding:14
                background: Rectangle { color: win.surface }
                ColumnLayout { anchors.fill:parent;spacing:8
                    Label { text:"PUBLICATION";font.pixelSize:11;font.letterSpacing:1.5;opacity:.5;Layout.topMargin:10;Layout.bottomMargin:12 }
                    WorkspaceButton { text:"Open…";Layout.fillWidth:true;onClicked:guarded(function(){openFolder.open()}) }
                    WorkspaceButton { text:"Create…";Layout.fillWidth:true;onClicked:guarded(function(){createDialog.open()}) }
                    Item { implicitHeight:16 }
                    Repeater { model:[{key:"all",label:"All articles"},{key:"draft",label:"Drafts"},{key:"ready",label:"Ready"},{key:"published",label:"Published"}]
                        delegate:WorkspaceButton { required property var modelData;text:modelData.label;flat:true;highlighted:filter===modelData.key;Layout.fillWidth:true;onClicked:filter=modelData.key }
                    }
                    Item { Layout.fillHeight:true }
                    Label { text:opened&&publication.state&&publication.state.pending?"Deployment needs recheck":opened&&publication.state&&publication.state.deployments.length?"Last deployment verified":"Local publication";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.65;font.pixelSize:12 }
                    WorkspaceButton { text:"History";enabled:opened;flat:true;Layout.fillWidth:true;onClicked:historyDialog.open() }
                    WorkspaceButton { text:"Settings";enabled:opened;flat:true;Layout.fillWidth:true;onClicked:guarded(function(){configEditor.text=publication.config_text;settingsDialog.open()}) }
                    Label { text:"OmaPress 0.1.0-rc.1";font.pixelSize:11;opacity:.4 }
                }
            }
            Rectangle { Layout.fillHeight:true;implicitWidth:1;color:backend.foreground;opacity:.13 }
            ColumnLayout {
                visible:opened;Layout.preferredWidth:win.width>=1100?260:210;Layout.maximumWidth:win.width>=1100?260:210;Layout.fillHeight:true;spacing:0
                RowLayout { Layout.fillWidth:true;Layout.margins:12
                    TextField { placeholderText:"Search articles…";Layout.fillWidth:true;onTextChanged:query=text;Accessible.name:"Search title, text, tags, series and date" }
                    ToolButton {font.capitalization:Font.MixedCase; text:"+";Accessible.name:"New article";onClicked:newArticle() }
                }
                ListView {
                    id:articleList;Layout.fillWidth:true;Layout.fillHeight:true;clip:true;model:shownArticles;spacing:1
                    delegate:ItemDelegate {
                        required property var modelData;width:articleList.width;height:116;highlighted:articlePath===modelData.path
                        leftPadding:20;rightPadding:16;topPadding:16;bottomPadding:16
                        background: Rectangle {
                            color: parent.highlighted ? win.surface : parent.hovered ? Qt.lighter(backend.background, 1.1) : "transparent"
                            Rectangle { width:3; height:parent.height-24; anchors.verticalCenter:parent.verticalCenter; color:backend.accent; visible:articlePath===modelData.path }
                        }
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
                WorkspaceButton {text:"Import Markdown…";Layout.fillWidth:true;Layout.margins:10;onClicked:guarded(function(){articleImport.open()})}
            }
            Rectangle {visible:opened;Layout.fillHeight:true;implicitWidth:1;color:backend.foreground;opacity:.13}
            ColumnLayout {
                visible:hasArticle;Layout.fillWidth:true;Layout.fillHeight:true;spacing:0
                RowLayout {Layout.fillWidth:true;Layout.margins:20;spacing:8
                    RowLayout { spacing:2
                        WorkspaceButton { objectName:"markdownTab";text:"Markdown";checkable:true;checked:!showPreview;highlighted:checked;onClicked:{showPreview=false;editor.forceActiveFocus()} }
                        WorkspaceButton { objectName:"previewTab";text:"Preview";checkable:true;checked:showPreview;highlighted:checked;onClicked:{showPreview=true;findBar.visible=false;renderNow()} }
                    }
                    Item {Layout.fillWidth:true}
                    Label {visible:win.width>=1100;text:dirty?"Unsaved changes":"Saved locally";font.pixelSize:12;opacity:.55}
                    WorkspaceButton {text:"Save";enabled:dirty;onClicked:save()}
                    WorkspaceButton {text:"Details";flat:true;onClicked:metadataDialog.open()}
                    WorkspaceButton {text:"More";flat:true;onClicked:articleMenu.open()
                        Menu {id:articleMenu
                            MenuItem {text:"Check publication";onTriggered:guarded(function(){backend.request("validate",{},"validate")})}
                            MenuItem {text:"Preview website";onTriggered:guarded(function(){backend.startPreview(true)})}
                            MenuItem {text:"Export for X";onTriggered:{renderNow();exportDialog.open()}}
                        }
                    }
                }
                TextField {visible:!showPreview;text:meta.title||"";placeholderText:meta.title?"":"Your editorial title";font.pixelSize:27;font.bold:true;Layout.fillWidth:true;Layout.leftMargin:40;Layout.rightMargin:40;background:Item{} onTextEdited:setMeta("title",text);Accessible.name:"Editorial title"}
                TextField {visible:!showPreview;text:meta.summary||"";placeholderText:meta.summary?"":"A short summary for the site and feeds";Layout.fillWidth:true;Layout.leftMargin:40;Layout.rightMargin:40;background:Item{} onTextEdited:setMeta("summary",text);Accessible.name:"Article summary"}
                RowLayout {visible:!showPreview;Layout.fillWidth:true;Layout.leftMargin:32;Layout.rightMargin:32
                    ToolButton {font.capitalization:Font.MixedCase;text:"H2";Accessible.name:"Insert heading";onClicked:insertMarkup("\n## ","\n")}
                    ToolButton {font.capitalization:Font.MixedCase;text:"B";font.bold:true;Accessible.name:"Bold";onClicked:insertMarkup("**","**")}
                    ToolButton {font.capitalization:Font.MixedCase;text:"I";font.italic:true;Accessible.name:"Italic";onClicked:insertMarkup("*","*")}
                    ToolButton {font.capitalization:Font.MixedCase;text:"Link";onClicked:insertMarkup("[","](https://)")}
                    ToolButton {font.capitalization:Font.MixedCase;text:"List";onClicked:insertMarkup("\n- ","\n")}
                    ToolButton {font.capitalization:Font.MixedCase;text:"Code";onClicked:insertMarkup("\n```\n","\n```\n")}
                    Item {Layout.fillWidth:true}
                    ToolButton {font.capitalization:Font.MixedCase;text:"Undo";enabled:editor.canUndo;onClicked:editor.undo()}
                    ToolButton {font.capitalization:Font.MixedCase;text:"Redo";enabled:editor.canRedo;onClicked:editor.redo()}
                }
                RowLayout {id:findBar;visible:false;Layout.fillWidth:true;Layout.margins:12
                    TextField {id:findField;placeholderText:"Find in article";Layout.fillWidth:true;onAccepted:findNext();Accessible.name:"Find in article"}
                    WorkspaceButton {text:"Next";onClicked:findNext()}
                    ToolButton {font.capitalization:Font.MixedCase;text:"×";Accessible.name:"Close find";onClicked:findBar.visible=false}
                }
                Item {
                    Layout.fillWidth:true;Layout.fillHeight:true
                    ScrollView {
                        anchors.fill:parent;visible:!showPreview;id:markdownView;objectName:"markdownView";clip:true
                        Flickable { clip:true; boundsBehavior:Flickable.StopAtBounds
                        TextArea.flickable: TextArea {id:editor;textFormat:TextEdit.PlainText;wrapMode:TextEdit.Wrap;selectByMouse:true;font.family:"monospace";font.pixelSize:16;leftPadding:40;rightPadding:40;topPadding:24;bottomPadding:80;placeholderText:"Paste your article or start writing in Markdown…";onTextChanged:changed();Accessible.name:"Markdown article body";background:Item{}}
                        }
                    }
                    ScrollView {
                        anchors.fill:parent;visible:showPreview;id:previewView;objectName:"previewView";clip:true
                        Flickable { clip:true; boundsBehavior:Flickable.StopAtBounds
                        TextArea.flickable: TextArea {id:previewText;text:"<h1>"+escapeHtml(meta.title||"Untitled article")+"</h1><p>"+escapeHtml(meta.summary)+"</p><br>"+rendered;textFormat:TextEdit.RichText;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;font.family:win.font.family;font.pixelSize:18;leftPadding:40;rightPadding:40;topPadding:24;bottomPadding:80;onLinkActivated:function(link){backend.openUrl(link)} Accessible.name:"Rendered article preview";background:Item{}}
                        }
                    }
                }
                Label {text:(editor.text.trim()?editor.text.trim().split(/\s+/).length:0)+" words · "+(showPreview?"Preview":"Markdown")+" · "+(meta.series||"");font.pixelSize:11;opacity:.5;Layout.margins:12}
            }
            Item {visible:!hasArticle;Layout.fillWidth:true;Layout.fillHeight:true
                ColumnLayout {anchors.centerIn:parent;width:Math.min(parent.width-64,480);spacing:18
                    Label {text:opened?"Make room for your next story.":"A home for your publication.";font.pixelSize:32;font.bold:true;wrapMode:Text.Wrap;Layout.fillWidth:true}
                    Label {text:opened?"Write locally, review the site, then publish. Your articles and feeds stay yours.":"Keep your articles in a folder you own. Build a website and feeds, then take your words to X.";font.pixelSize:17;wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.6}
                    WorkspaceButton {text:opened?"Write an article":"Create a publication";highlighted:true;onClicked:opened?newArticle():createDialog.open()}
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

    Dialog {id:connectionsDialog;objectName:"connectionsDialog";title:"Connections";modal:true;anchors.centerIn:parent;width:600;standardButtons:Dialog.Close
        ColumnLayout {anchors.fill:parent;spacing:12
            Button {text:"Configure Substack Gateway…";onClicked:gatewayConfigDialog.open()}
            Label {text:"X Articles";font.pixelSize:22;font.bold:true}
            Label {text:"Sign in with a public Native App client ID from the X developer console. Register this exact callback URL:";wrapMode:Text.Wrap;Layout.fillWidth:true}
            TextField {text:"http://127.0.0.1:39123/callback";readOnly:true;Layout.fillWidth:true;Accessible.name:"X callback URL"}
            TextField {id:xClientId;placeholderText:"X public client ID";Layout.fillWidth:true;Accessible.name:"X public client ID"}
            Label {text:backend.busy?"Waiting for the current operation. Browser sign-in expires after three minutes.":(xConnection.connected?"Connected. Credentials are stored in your desktop keyring.":"Check connection status or sign in.");wrapMode:Text.Wrap;Layout.fillWidth:true}
            RowLayout {
                Button {text:"Connect X";enabled:!backend.busy&&xClientId.text.length>0;onClicked:backend.request("x-connect",{client_id:xClientId.text})}
                Button {text:"Check status";enabled:!backend.busy;onClicked:backend.request("x-status")}
                Button {text:"Disconnect";enabled:!backend.busy;onClicked:backend.request("x-disconnect")}
            }
            Label {text:"Website: use publication setup to connect your GitHub repository. Substack uses your browser session. Install the bundled companion using install-companion.py, then load the companion folder in your browser’s extension manager. See docs/substack-companion.md.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.7}
        }
    }
    Dialog {id:distributionDialog;objectName:"distributionDialog";title:"Publish article";modal:true;anchors.centerIn:parent;width:760;height:Math.min(win.height-60,800);standardButtons:Dialog.Close
        ScrollView {anchors.fill:parent;clip:true
            ColumnLayout {width:distributionDialog.availableWidth-24;spacing:14
                Label {text:distribution.title||"";font.pixelSize:24;font.bold:true;wrapMode:Text.Wrap;Layout.fillWidth:true}
                Label {text:"One saved article. A separate result for each destination.";opacity:.7}
                Label {text:distribution.ready?"Reviewed saved version ready for publication.":"Mark this article Ready and resolve publication checks before publishing.";wrapMode:Text.Wrap;Layout.fillWidth:true;color:backend.accent}
                Repeater {model:distribution.targets||[];delegate:Frame {required property var modelData;Layout.fillWidth:true
                    ColumnLayout {width:parent.width
                        Label {text:({website:"Website",x:"X Articles",substack:"Substack"})[modelData.target];font.bold:true;font.pixelSize:18}
                        Label {text:modelData.status.replace(/_/g," ");color:backend.accent}
                        Button {visible:!!(modelData.receipt&&modelData.receipt.url);text:"Open recorded destination";onClicked:backend.openUrl(modelData.receipt.url)}
                    }
                }}
                RowLayout {
                    CheckBox {id:batchWebsite;text:"Website";checked:true}
                    CheckBox {id:batchSubstack;text:"Substack draft"}
                    CheckBox {id:batchX;text:"X Articles";enabled:!!distribution.x_api_supported}
                    Button {text:"Review selected destinations…";enabled:distribution.ready&&!backend.busy&&(batchWebsite.checked||batchSubstack.checked||(batchX.checked&&batchX.enabled));onClicked:{var a=distributionArgs();a.substack=batchSubstack.checked;a.website=batchWebsite.checked;a.x=batchX.checked&&batchX.enabled;backend.request("distribution-review",a,"distribution-review")}}
                }
                Label {text:distribution.website_scope||"";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.7}
                Button {text:"Review website deployment…";enabled:distribution.ready&&!backend.busy;onClicked:{rollbackId="";backend.request("publish-plan",{expected_source_hash:distribution.source_hash},"plan")}}
                Label {text:"X Articles · API preview";font.bold:true}
                Label {text:distribution.x_api_issue||"Rich text, tables, code, and local PNG/JPEG artwork are supported. Live account acceptance is pending.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.7}
                RowLayout {
                    Button {text:"Create X draft";enabled:distribution.ready&&distribution.x_api_supported&&!backend.busy;onClicked:backend.request("x-draft",distributionArgs(),"x-action")}
                    Button {text:"Publish on X…";enabled:distribution.ready&&distribution.x_api_supported&&!backend.busy;onClicked:xConfirm.open()}
                    Button {text:"Copy rich article";enabled:!!distribution.x;onClicked:{backend.copyArticle(distribution.x.html,distribution.x.text);say("Rich article copied. Upload artwork separately in X.")}}
                }
                Button {text:"Open X Articles";onClicked:backend.openUrl("https://x.com/compose/articles")}
                Label {text:"Substack · API scheduling (optional)";font.bold:true}
                Label {text:"Uses your self-hosted Substack Gateway and session credentials. Prepare a draft, inspect its formatting in Substack, then submit the release time. Substack handles an accepted schedule while your laptop is off.";wrapMode:Text.Wrap;Layout.fillWidth:true}
                RowLayout {
                    Button {text:"Configure gateway…";onClicked:gatewayConfigDialog.open()}
                    Button {text:"Prepare API draft";enabled:distribution.ready&&!backend.busy;onClicked:backend.request("substack-gateway-draft",distributionArgs(),"gateway-action")}
                    Button {text:"Open Substack draft editor";enabled:!!gatewayReceipt.url;onClicked:backend.openUrl(gatewayReceipt.url)}
                }
                Label {text:gatewayReceipt.status?"Substack: "+gatewayReceipt.status.replace(/_/g," "):"No API draft prepared";wrapMode:Text.Wrap;Layout.fillWidth:true;color:backend.accent}
                Label {visible:!!gatewayReceipt.scheduled_at;text:"Release: "+(gatewayReceipt.scheduled_at||"")+" · post: "+(gatewayReceipt.post_audience||"")+" · email: "+(gatewayReceipt.email_audience||"");wrapMode:Text.Wrap;Layout.fillWidth:true}
                TextField {id:gatewayTime;placeholderText:"Release: YYYY-MM-DD HH:MM";Layout.fillWidth:true;Accessible.name:"Substack release local time"}
                TextField {id:gatewayZone;text:publication.config?publication.config.timezone:"Europe/London";Layout.fillWidth:true;Accessible.name:"Substack release timezone"}
                RowLayout {
                    Label {text:"Post audience"}
                    ComboBox {id:gatewayPostAudience;model:["everyone","only_paid"];Accessible.name:"Substack post audience"}
                    Label {text:"Email audience"}
                    ComboBox {id:gatewayEmailAudience;model:["everyone","only_paid"];Accessible.name:"Substack email audience"}
                }
                CheckBox {id:gatewayReviewed;text:"I reviewed the Substack draft and the audience/email choices";enabled:gatewayReceipt.status==="gateway_draft"}
                RowLayout {
                    Button {text:"Schedule on Substack";highlighted:true;enabled:gatewayReviewed.checked&&gatewayReceipt.status==="gateway_draft"&&!backend.busy;onClicked:{var a=distributionArgs();a.local_time=gatewayTime.text;a.timezone=gatewayZone.text;a.post_audience=gatewayPostAudience.currentText;a.email_audience=gatewayEmailAudience.currentText;a.reviewed_draft_hash=gatewayReceipt.review_hash;a.reviewed_in_substack=gatewayReviewed.checked;backend.request("substack-gateway-schedule",a,"gateway-action")}}
                    Button {text:"Cancel Substack schedule";enabled:gatewayReceipt.status==="scheduled"&&!backend.busy;onClicked:backend.request("substack-gateway-cancel",distributionArgs(),"gateway-action")}
                }
                Label {text:"An acknowledged schedule is not proof of publication. If an operation times out, inspect Substack before proceeding. Cover artwork is placed at the top of the article body; check the separate social preview in Substack.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.7}
                Label {text:"Substack · browser companion";font.bold:true}
                Label {text:"Prepare the saved article and artwork, then use the OmaPress companion to fill a blank Substack draft. Review the audience and email delivery in Substack before publishing.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.7}
                Button {text:"Prepare Substack draft";enabled:distribution.ready&&!backend.busy;onClicked:backend.request("substack-prepare",distributionArgs())}
                RowLayout {
                    Button {text:"Copy title";onClicked:backend.copyText(distribution.substack.title)}
                    Button {text:"Copy subtitle";onClicked:backend.copyText(distribution.substack.subtitle)}
                    Button {text:"Copy article body";onClicked:backend.copyArticle(distribution.substack.html,distribution.substack.text)}
                    Button {text:"Open Substack";onClicked:backend.openUrl("https://substack.com/publish")}
                }
                Label {text:"Record a published article";font.bold:true}
                Label {text:"This records your confirmation, not independent provider verification.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.7}
                ComboBox {id:receiptTarget;model:["x","substack"];Accessible.name:"Published destination"}
                TextField {id:receiptUrl;placeholderText:"https://… published article URL";Layout.fillWidth:true;Accessible.name:"Published article URL"}
                Button {text:"Record published URL";enabled:receiptUrl.text!==""&&!backend.busy;onClicked:{var a=distributionArgs();a.target=receiptTarget.currentText;a.url=receiptUrl.text;backend.request("distribution-confirm",a,"confirm-destination")}}
                Label {text:"Use Connections to sign in to X. Tokens remain in your desktop keyring.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.6}
            }
        }
    }
    Dialog {id:batchConfirm;title:"Publish selected destinations?";modal:true;anchors.centerIn:parent;width:600;standardButtons:Dialog.Cancel
        ColumnLayout {width:parent.width;spacing:12
            Label {text:distributionReview.title||"";font.bold:true;wrapMode:Text.Wrap;Layout.fillWidth:true}
            Label {text:"Publish to: "+(distributionReview.website?"Website ":"")+(distributionReview.x?"X Articles ":"")+(distributionReview.substack?"Prepare Substack draft":"");wrapMode:Text.Wrap;Layout.fillWidth:true}
            Label {visible:!!distributionReview.website;text:"Website repository: "+(distributionReview.site?distributionReview.site.repository:"")+"\nAll Ready/Published articles are included in the website deployment.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            Label {text:"Substack preparation adds this version to your local browser outbox. Final publishing happens in Substack. If one destination fails, other results are retained.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            Button {text:"Publish reviewed version";enabled:!backend.busy;onClicked:{var r=distributionReview;backend.request("distribution-publish",{article:r.article,expected_source_hash:r.source_hash,website:r.website,x:r.x,substack:r.substack,expected_remote_head:r.site?r.site.expected_remote_head:""},"distribution-publish");batchConfirm.close()}}
        }
    }
    Dialog {id:xConfirm;title:"Publish on X?";modal:true;anchors.centerIn:parent;width:500;standardButtons:Dialog.Cancel
        ColumnLayout {width:parent.width
            Label {text:"Make this reviewed article public on X: "+(distribution.title||"");wrapMode:Text.Wrap;Layout.fillWidth:true}
            Label {text:"Website and Substack are separate destinations. This action publishes only on X.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            Button {text:"Publish reviewed article on X";enabled:!backend.busy;onClicked:{xConfirm.close();backend.request("x-publish",distributionArgs(),"x-action")}}
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
    Dialog {id:gatewayConfigDialog;objectName:"gatewayConfigDialog";title:"Substack Gateway connection";modal:true;anchors.centerIn:parent;width:620;standardButtons:Dialog.Close
        ColumnLayout {width:parent.width;spacing:10
            Label {text:"Connect your own Substack Gateway OSS server. It receives your Substack session credentials. Use HTTPS or a local loopback server. Credentials are stored in your keyring.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            TextField {id:gatewayUrl;placeholderText:"https://your-gateway.example or http://127.0.0.1:5001";Layout.fillWidth:true;Accessible.name:"Gateway URL"}
            TextField {id:gatewayPublication;placeholderText:"https://your-publication.substack.com";Layout.fillWidth:true;Accessible.name:"Substack publication URL"}
            TextField {id:gatewaySession;placeholderText:"substack.sid session value";echoMode:TextInput.Password;Layout.fillWidth:true;Accessible.name:"Substack session credential"}
            TextField {id:gatewayConnectSession;placeholderText:"connect.sid session value (if available)";echoMode:TextInput.Password;Layout.fillWidth:true;Accessible.name:"Substack connect session credential"}
            RowLayout {
                Button {text:"Save connection";enabled:!backend.busy;onClicked:backend.request("substack-gateway-connect",{gateway_url:gatewayUrl.text,publication_url:gatewayPublication.text,substack_sid:gatewaySession.text,connect_sid:gatewayConnectSession.text},"gateway-connect")}
                Button {text:"Show saved connection";enabled:!backend.busy;onClicked:backend.request("substack-gateway-status",{},"gateway-status")}
            }
            Label {text:gatewayConnection.configured?gatewayConnection.publication_url+" via "+gatewayConnection.gateway_url:"No configuration loaded";textFormat:Text.PlainText;wrapMode:Text.Wrap;Layout.fillWidth:true}
        }
        onClosed:{gatewaySession.text="";gatewayConnectSession.text=""}
    }
    Dialog {id:intakeDialog;objectName:"intakeDialog";title:"Add article from ChatGPT";modal:true;anchors.centerIn:parent;width:720;height:Math.min(win.height-60,720);standardButtons:Dialog.Cancel
        ColumnLayout {anchors.fill:parent
            Label {text:"Paste the finished article as plain text or Markdown. It will be saved as a draft.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            TextField {id:intakeTitle;placeholderText:"Article title";Layout.fillWidth:true;Accessible.name:"Imported article title"}
            TextField {id:intakeSummary;placeholderText:"Short summary";Layout.fillWidth:true;Accessible.name:"Imported article summary"}
            ComboBox {id:intakeSeries;model:seriesIds;Accessible.name:"Article series"}
            ScrollView {Layout.fillWidth:true;Layout.fillHeight:true;TextArea {id:intakeBody;placeholderText:"Paste your article here…";textFormat:TextEdit.PlainText;wrapMode:TextEdit.Wrap;Accessible.name:"Article text to import"}}
            Button {text:"Save as draft";highlighted:true;enabled:intakeTitle.text.trim()!==""&&intakeBody.text.trim()!==""&&!backend.busy;onClicked:{backend.request("intake",{title:intakeTitle.text,summary:intakeSummary.text,body:intakeBody.text,series:intakeSeries.currentText,expected_source_hash:sourceHash},"import");intakeDialog.close()}}
        }
    }
    Dialog {id:queueDialog;objectName:"queueDialog";title:"Publishing queue";modal:true;anchors.centerIn:parent;width:800;height:Math.min(win.height-50,820);standardButtons:Dialog.Close
        ScrollView {anchors.fill:parent;clip:true;ColumnLayout {width:queueDialog.availableWidth-24;spacing:12
            CheckBox {id:remoteEnabled;text:"Use an always-on SSH worker";onToggled:{queueState={jobs:[]};remoteSource="";queueId=""}}
            Label {text:remoteEnabled.checked?"Accepted jobs run on your worker while this laptop is off. The worker needs its own credentials and timer.":"Local jobs require this computer to be awake and the worker timer to be installed. The app alone does not run the queue.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            TextField {id:workerHost;visible:remoteEnabled.checked;placeholderText:"SSH host alias, e.g. omapress-worker";Layout.fillWidth:true;onTextChanged:{remoteSource="";queueState={jobs:[]}} Accessible.name:"Worker SSH alias"}
            TextField {id:workerPath;visible:remoteEnabled.checked;placeholderText:"Absolute worker publication path, e.g. /srv/omapress/publication";Layout.fillWidth:true;onTextChanged:{remoteSource="";queueState={jobs:[]}} Accessible.name:"Worker publication path"}
            RowLayout {
                Button {text:"Refresh queue";enabled:!backend.busy;onClicked:queueRequest("queue-list",{},"queue-list")}
                Button {text:"Upload reviewed publication";visible:remoteEnabled.checked;enabled:!backend.busy&&!dirty;onClicked:queueRequest("queue-upload",{expected_source_hash:sourceHash,expected_remote_source_hash:remoteSource},"queue-uploaded")}
            }
            Label {text:queueState.worker?"Worker last ran: "+queueState.worker.last_started_at:"No worker run recorded. Install and start its timer before relying on this queue.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.7}
            Label {text:"Schedule: "+(meta.title||"Select an article first");font.bold:true;wrapMode:Text.Wrap;Layout.fillWidth:true}
            Label {text:"Save the article, mark it Ready and resolve Checks. Website scheduling deploys ALL Ready/Published articles together. Keep later website articles in Draft. Changes to this publication block its queued jobs; cancel and review them again.";wrapMode:Text.Wrap;Layout.fillWidth:true}
            TextField {id:queueTime;placeholderText:"YYYY-MM-DD HH:MM";Layout.fillWidth:true;onTextChanged:queueId="";Accessible.name:"Scheduled local date and time"}
            TextField {id:queueZone;text:publication.config?publication.config.timezone:"Europe/London";Layout.fillWidth:true;onTextChanged:queueId="";Accessible.name:"Schedule timezone"}
            RowLayout {
                CheckBox {id:queueWebsite;text:"Website";onToggled:queueId=""}
                CheckBox {id:queueX;text:"X Article";onToggled:queueId=""}
            }
            Label {text:"Substack: use API scheduling in Publish, or the browser handoff. Substack handles an accepted schedule; it is not submitted to this worker.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.7}
            Button {text:"Schedule saved version";highlighted:true;enabled:hasArticle&&!dirty&&!backend.busy&&(queueWebsite.checked||queueX.checked);onClicked:{if(!queueId)queueId=backend.newJobId();var targets=[];if(queueWebsite.checked)targets.push("website");if(queueX.checked)targets.push("x");queueRequest("queue-add",{id:queueId,article:articlePath,expected_source_hash:sourceHash,local_time:queueTime.text,timezone:queueZone.text,targets:targets},"queue-added")}}
            Repeater {model:queueState.jobs||[];delegate:Frame {required property var modelData;Layout.fillWidth:true;ColumnLayout {width:parent.width
                Label {text:modelData.title;wrapMode:Text.Wrap;Layout.fillWidth:true;font.bold:true}
                Label {text:modelData.at+" · "+modelData.timezone+" · "+modelData.targets.join(", ");wrapMode:Text.Wrap;Layout.fillWidth:true}
                Label {text:modelData.status.replace(/_/g," ")+ (modelData.message?" — "+modelData.message:"");wrapMode:Text.Wrap;Layout.fillWidth:true;color:backend.accent}
                Label {visible:Object.keys(modelData.results||{}).length>0;text:JSON.stringify(modelData.results,null,2);textFormat:Text.PlainText;wrapMode:Text.WrapAnywhere;Layout.fillWidth:true;font.pixelSize:12}
                Button {text:"Reconcile recorded results";visible:modelData.status==="needs_review";enabled:!backend.busy;onClicked:queueRequest("queue-reconcile",{id:modelData.id},"queue-list")}
                Button {text:"Cancel job";visible:modelData.status==="queued"||modelData.status==="blocked";enabled:!backend.busy;onClicked:queueRequest("queue-cancel",{id:modelData.id},"queue-cancelled")}
            }}}
        }}
    }
    Dialog {id:deleteDialog;title:"Delete this draft?";modal:true;anchors.centerIn:parent;standardButtons:Dialog.Cancel|Dialog.Ok
        Label {text:"The draft will be moved to local recovery storage."}
        onAccepted:{metadataDialog.close();backend.request("delete",{article:articlePath,expected_source_hash:sourceHash},"delete")}
    }
}
