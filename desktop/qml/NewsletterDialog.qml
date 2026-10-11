import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Dialog {
    id: dialog
    objectName: "newsletterDialog"
    title: "Newsletters"
    modal: true
    anchors.centerIn: parent
    width: Math.min(parent.width - 40, 820)
    height: Math.min(parent.height - 40, 820)
    standardButtons: Dialog.Close
    property string article: ""
    property string reviewedSourceHash: ""
    property string timezone: "Europe/London"
    property var newsletters: []
    property var review: ({})
    property var result: ({})
    property var connection: ({})
    property string notice: "Connect your newsletter service, then choose a newsletter."
    property string testId: ""
    property int generation: 0
    readonly property string newsletterId: newsletters.length && selection.currentIndex >= 0 ? newsletters[selection.currentIndex].id : ""
    function resetReview() { generation++; review={};result={};reviewed.checked=false;testId="" }
    function request(command,args,tag) { backend.request(command,args,"newsletter-ui:"+generation+":"+tag) }
    function args() { return {article:article,expected_source_hash:reviewedSourceHash,newsletter_id:newsletterId} }
    function refresh() {request("newsletter-list",{},"list")}
    onArticleChanged: resetReview()
    onNewsletterIdChanged: resetReview()
    onReviewedSourceHashChanged: {reviewed.checked=false}
    onClosed: {token.text="";resetReview()}
    Connections {
        target: backend
        function onFailed(tag,error) {
            if(tag.indexOf("newsletter-ui:"+dialog.generation+":")!==0)return;
            token.text="";dialog.notice=error;reviewed.checked=false
        }
        function onResult(tag,v) {
            if(tag.indexOf("newsletter-ui:"+dialog.generation+":")!==0)return;
            var action=tag.split(":")[2];token.text=""
            if(action==="list"||action==="connect") {
                dialog.connection=v;serviceUrl.text=v.service_url||serviceUrl.text;dialog.newsletters=v.newsletters||[];
                dialog.notice=v.transport==="capture"?"Capture mode: email stays on the server. No messages are sent.":"Connected to your newsletter service."
            } else if(action==="create") {dialog.notice="Newsletter created.";dialog.refresh()}
            else if(action==="review") {dialog.review=v;dialog.result={};reviewed.checked=false;dialog.notice="Review the saved email and subscriber count before sending."}
            else if(action==="status") {dialog.result=v;dialog.review={};reviewed.checked=false;dialog.notice="Campaign status refreshed. Complete means processing finished; check delivery counts."}
            else if(action==="test") {dialog.notice="Test queued for "+v.recipient+(v.transport==="capture"?" (capture mode; no email sent)":"");dialog.testId=""}
            else if(action==="contacts") {csvText.text=v.csv;dialog.notice=v.active+" active subscribers; "+v.total+" total records. CSV ready to copy."}
            else if(action==="import") {dialog.notice=v.added+" subscriptions added. Existing opt-outs preserved.";consent.checked=false}
        }
    }
    ScrollView {
        anchors.fill: parent
        clip: true
        ColumnLayout {
            width: dialog.availableWidth-24
            spacing: 12
            Label {text:dialog.notice;textFormat:Text.PlainText;wrapMode:Text.Wrap;Layout.fillWidth:true}
            GroupBox {
                title:"Connection";Layout.fillWidth:true
                ColumnLayout {anchors.fill:parent
                    TextField {id:serviceUrl;placeholderText:"https://newsletter.example.com";Layout.fillWidth:true;Accessible.name:"Newsletter service URL"}
                    TextField {id:token;placeholderText:"Service API token";echoMode:TextInput.Password;Layout.fillWidth:true;Accessible.name:"Newsletter API token"}
                    RowLayout {
                        Button {text:"Connect";enabled:!backend.busy&&token.text.length>=32;onClicked:dialog.request("newsletter-connect",{service_url:serviceUrl.text,api_token:token.text},"connect")}
                        Button {text:"Load saved connection";enabled:!backend.busy;onClicked:dialog.refresh()}
                    }
                }
            }
            ComboBox {id:selection;model:dialog.newsletters;textRole:"name";Layout.fillWidth:true;Accessible.name:"Newsletter audience";enabled:!backend.busy}
            Label {text:dialog.connection.transport==="capture"?"CAPTURE MODE — no real delivery":dialog.connection.sending_enabled?"Sending enabled":"Sending disabled";font.bold:true}
            GroupBox {
                title:"Selected article";Layout.fillWidth:true
                ColumnLayout {anchors.fill:parent
                    Label {text:dialog.article||"Open an article to prepare a campaign.";wrapMode:Text.Wrap;Layout.fillWidth:true}
                    Label {text:"Preparing a draft uploads its artwork to publicly accessible image URLs on your service.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:0.7}
                    Flow {Layout.fillWidth:true;spacing:8
                        Button {text:"Prepare saved article";enabled:!!dialog.article&&!!dialog.newsletterId&&!backend.busy;onClicked:dialog.request("newsletter-prepare",dialog.args(),"review")}
                        Button {text:"Review campaign";enabled:!!dialog.article&&!!dialog.newsletterId&&!backend.busy;onClicked:dialog.request("newsletter-review",dialog.args(),"review")}
                        Button {text:"Refresh status";enabled:!!dialog.article&&!!dialog.newsletterId&&!backend.busy;onClicked:dialog.request("newsletter-refresh",dialog.args(),"status")}
                    }
                    Label {visible:!!dialog.result.campaign;text:dialog.result.campaign?"Status: "+dialog.result.campaign.status+"\n"+JSON.stringify(dialog.result.counts||{}):"";wrapMode:Text.Wrap;Layout.fillWidth:true}
                    Label {visible:!!dialog.review.campaign;text:dialog.review.campaign?dialog.review.campaign.subject+"\n"+dialog.review.newsletter.name+" · "+dialog.review.audience_count+" active subscribers\nFrom: "+dialog.review.newsletter.from_email+"\nReply to: "+dialog.review.newsletter.reply_to:"";wrapMode:Text.Wrap;Layout.fillWidth:true;font.bold:true}
                    TextArea {visible:!!dialog.review.html;text:dialog.review.html||"";textFormat:TextEdit.RichText;readOnly:true;wrapMode:TextEdit.Wrap;Layout.fillWidth:true;Layout.preferredHeight:visible?260:0;Accessible.name:"Newsletter preview"}
                    Label {visible:!!dialog.review.html;text:"Preview uses the desktop renderer. Send yourself a test to check the final email layout.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.7}
                    Button {text:"Send test to "+(dialog.review.test_recipient||"configured test address");enabled:!!dialog.review.campaign&&!backend.busy;onClicked:{if(!dialog.testId)dialog.testId=backend.newJobId();var a=dialog.args();a.revision=dialog.review.campaign.revision;a.request_id=dialog.testId;dialog.request("newsletter-test",a,"test")}}
                    CheckBox {id:reviewed;text:"I reviewed this email and its subscriber audience";enabled:!!dialog.review.campaign&&dialog.review.campaign.status==="draft"}
                    CheckBox {id:sendNow;text:"Send now";checked:true;onCheckedChanged:reviewed.checked=false}
                    TextField {id:releaseTime;visible:!sendNow.checked;placeholderText:"YYYY-MM-DD HH:MM";Layout.fillWidth:true;Accessible.name:"Newsletter scheduled time";onTextChanged:reviewed.checked=false}
                    TextField {id:releaseZone;visible:!sendNow.checked;text:dialog.timezone;Layout.fillWidth:true;Accessible.name:"Newsletter timezone";onTextChanged:reviewed.checked=false}
                    RowLayout {
                        Button {text:sendNow.checked?"Send newsletter":"Schedule newsletter";highlighted:true;enabled:reviewed.checked&&!backend.busy;onClicked:{var a=dialog.args();a.revision=dialog.review.campaign.revision;a.audience_hash=dialog.review.audience_hash;a.reviewed=true;a.send_now=sendNow.checked;a.local_time=releaseTime.text;a.timezone=releaseZone.text;dialog.request("newsletter-schedule",a,"status");reviewed.checked=false}}
                        Button {text:"Cancel remaining delivery";enabled:!!dialog.article&&!!dialog.newsletterId&&!backend.busy;onClicked:cancelConfirmation.open()}
                    }
                    Label {text:"The server handles accepted schedules while your laptop is off. Cancellation stops queued messages; an email already being sent cannot be recalled.";wrapMode:Text.Wrap;Layout.fillWidth:true;opacity:.7}
                }
            }
            GroupBox {title:"Subscribers";Layout.fillWidth:true
                ColumnLayout {anchors.fill:parent
                    Label {text:"Paste CSV with an email column to import an existing opted-in list. Each newsletter has its own preferences and unsubscribe link.";wrapMode:Text.Wrap;Layout.fillWidth:true}
                    TextArea {id:csvText;placeholderText:"email\nreader@example.com";Layout.fillWidth:true;Layout.preferredHeight:100;wrapMode:TextEdit.Wrap;Accessible.name:"Subscriber CSV"}
                    TextField {id:consentSource;placeholderText:"Where and when these readers opted in";Layout.fillWidth:true;Accessible.name:"Subscriber consent source"}
                    CheckBox {id:consent;text:"These readers already opted in to this newsletter"}
                    Flow {Layout.fillWidth:true;spacing:8
                        Button {text:"Import subscribers";enabled:consent.checked&&!!dialog.newsletterId&&!backend.busy;onClicked:dialog.request("newsletter-import",{newsletter_id:dialog.newsletterId,csv:csvText.text,consent_attested:consent.checked,consent_source:consentSource.text},"import")}
                        Button {text:"Export subscriber CSV";enabled:!!dialog.newsletterId&&!backend.busy;onClicked:dialog.request("newsletter-contacts",{newsletter_id:dialog.newsletterId},"contacts")}
                        Button {text:"Copy CSV";onClicked:backend.copyText(csvText.text)}
                    }
                }
            }
            GroupBox {title:"Create a newsletter";Layout.fillWidth:true
                ColumnLayout {anchors.fill:parent
                    TextField {id:newName;placeholderText:"Newsletter name";Layout.fillWidth:true;Accessible.name:"Newsletter name"}
                    TextField {id:newSlug;placeholderText:"newsletter-slug";Layout.fillWidth:true;Accessible.name:"Newsletter slug"}
                    TextField {id:fromEmail;placeholderText:"From email on your verified SES domain";Layout.fillWidth:true;Accessible.name:"Sender email"}
                    TextField {id:replyEmail;placeholderText:"Reply-to email";Layout.fillWidth:true;Accessible.name:"Reply-to email"}
                    TextField {id:postal;placeholderText:"Sender postal address for email footer";Layout.fillWidth:true;Accessible.name:"Sender postal address"}
                    Button {text:"Create newsletter";enabled:!backend.busy;onClicked:dialog.request("newsletter-create",{slug:newSlug.text,name:newName.text,from_email:fromEmail.text,reply_to:replyEmail.text,postal_address:postal.text},"create")}
                    Button {text:"Open signup page";enabled:!!dialog.newsletterId&&serviceUrl.text!=="";onClicked:backend.openUrl(serviceUrl.text.replace(/\/$/,"")+"/subscribe/"+dialog.newsletters[selection.currentIndex].slug)}
                }
            }
        }
    }
    Dialog {id:cancelConfirmation;title:"Cancel remaining newsletter delivery?";modal:true;anchors.centerIn:parent;standardButtons:Dialog.Cancel|Dialog.Ok
        Label {text:"Queued emails will be cancelled. Messages already sent or in flight cannot be recalled.";wrapMode:Text.Wrap;width:360}
        onAccepted:dialog.request("newsletter-cancel",dialog.args(),"status")
    }
}
