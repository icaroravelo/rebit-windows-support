#include "SequenceCanvas.h"
#include <cmath>

namespace {
constexpr int pageSize=16, maxFrames=65534, accentId=0x234567;
const juce::Colour ink{0xfff0f3ff}, muted{0xff8e99c8}, line{0xff2a3163}, inset{0xff050812};
juce::Font mono(float size) { return juce::Font(juce::FontOptions(juce::Font::getDefaultMonospacedFontName(),size,juce::Font::plain)); }
}
SequenceCanvas::SequenceCanvas(RebitProcessor& p):processor(p) {
    setWantsKeyboardFocus(true); setTitle("Visual sequence editor");
    setDescription("Draw steps with the mouse. Arrow keys select steps and adjust values. Drag LOOP and REL markers.");
    auto button=[&](juce::TextButton& b){b.getProperties().set("sequenceSmall",true);addAndMakeVisible(b);};
    const char* names[]={"VOLUME","PITCH","DUTY"};
    for(size_t i=0;i<3;++i){
        auto& tab=tabs[i];tab.setButtonText(names[i]);button(tab);
        tab.onClick=[this,i]{selectLane((int)i);};
    }
    button(loop);button(release);button(range);button(previous);button(next);
    loop.setClickingTogglesState(true);release.setClickingTogglesState(true);
    loop.setTooltip("Loop the main section. Drag LOOP to choose its first step.");
    release.setTooltip("Use the final four steps as a note-off tail. Drag REL to move the boundary.");
    range.setTooltip("Pitch drawing range: +/-12, +/-24, or the full MIDI sequence range. Arrow keys adjust by one unit.");
    loop.onClick=[this]{seed();data.loopStart=loop.getToggleState()?0:-1;dirty=true;commit();};
    release.onClick=[this]{
        seed();
        if(release.getToggleState()){
            if(data.values.size()==1)data.values.resize(5,0);
            data.releaseStart=juce::jmax(1,(int)data.values.size()-4);
            if(data.loopStart>=data.releaseStart)data.loopStart=data.releaseStart-1;
        }else data.releaseStart=-1;
        dirty=true;commit();
    };
    range.onClick=[this]{pitchRange=pitchRange==12?24:pitchRange==24?64:12;controls();repaint();};
    previous.onClick=[this]{page=juce::jmax(0,page-1);selectedStep=page*pageSize;controls();repaint();};
    next.onClick=[this]{page=juce::jmin(((int)data.values.size()-1)/pageSize,page+1);selectedStep=page*pageSize;controls();repaint();};
    for(auto* label:{&stepsLabel,&pageLabel,&valueLabel}){
        label->setFont(mono(10));label->setColour(juce::Label::textColourId,muted);
        label->setBorderSize(juce::BorderSize<int>(0));addAndMakeVisible(label);
    }
    stepsLabel.setText("STEPS",juce::dontSendNotification);
    pageLabel.setJustificationType(juce::Justification::centred);
    length.setName("Sequence length");length.setInputRestrictions(5,"0123456789");
    length.setFont(mono(11));length.setIndents(8,5);
    length.setTooltip("Number of frames, 0 to 65,534. Zero clears the selected lane.");
    length.onReturnKey=[this]{resizePattern();};length.onFocusLost=[this]{resizePattern();};addAndMakeVisible(length);
    refresh();startTimerHz(30);
}
SequenceCanvas::~SequenceCanvas(){stopTimer();}
juce::Colour SequenceCanvas::accent() const {return selectedLane==0?juce::Colour(0xff3dff7a):selectedLane==1?juce::Colour(0xff2ce4ff):juce::Colour(0xff5b7cff);}
int SequenceCanvas::minValue() const {return selectedLane==1?-pitchRange:0;}
int SequenceCanvas::maxValue() const {return selectedLane==0?15:selectedLane==1?juce::jmin(63,pitchRange):2;}
juce::Rectangle<float> SequenceCanvas::grid() const {return {0,50,(float)getWidth(),(float)getHeight()-50};}
float SequenceCanvas::stepX(int step) const {return (float)(step-page*pageSize)*grid().getWidth()/(float)pageSize;}
int SequenceCanvas::stepAt(float x) const {return page*pageSize+juce::jlimit(0,pageSize-1,(int)(x/grid().getWidth()*(float)pageSize));}
void SequenceCanvas::seed(){
    if(data.values.empty()){data.values.assign(16,(int16_t)(selectedLane==0?15:0));data.loopStart=0;data.releaseStart=-1;page=0;selectedStep=0;}
}
void SequenceCanvas::controls(){
    const auto colour=accent();
    for(size_t i=0;i<3;++i){tabs[i].setToggleState((int)i==selectedLane,juce::dontSendNotification);tabs[i].setColour(accentId,colour);}
    for(auto* button:{&loop,&release,&range,&previous,&next})button->setColour(accentId,colour);
    loop.setToggleState(data.loopStart>=0,juce::dontSendNotification);
    release.setToggleState(data.releaseStart>=0,juce::dontSendNotification);
    if(!length.hasKeyboardFocus(true))length.setText(juce::String((int)data.values.size()),false);
    range.setVisible(selectedLane==1);range.setButtonText(pitchRange==64?"FULL":"+/-"+juce::String(pitchRange));
    const int count=(int)data.values.size();page=juce::jlimit(0,juce::jmax(0,(count-1)/pageSize),page);
    previous.setEnabled(page>0);next.setEnabled((page+1)*pageSize<count);
    pageLabel.setText(count==0?"EMPTY":juce::String(page*pageSize+1)+" - "+juce::String(juce::jmin(count,(page+1)*pageSize))+" / "+juce::String(count),juce::dontSendNotification);
    const int value=selectedStep>=0&&selectedStep<count?(int)data.values[(size_t)selectedStep]:0;
    valueLabel.setText(count==0?"DRAW TO BEGIN":"STEP "+juce::String(selectedStep+1)+"  /  "+(value>0&&selectedLane==1?"+":"")+juce::String(value),juce::dontSendNotification);
}
void SequenceCanvas::selectLane(int lane){
    commit();selectedLane=juce::jlimit(0,2,lane);page=0;selectedStep=0;observedRevision=~uint64_t{};refresh();
    if(onLaneChanged)onLaneChanged(selectedLane);
}
void SequenceCanvas::selectLayer(int layer){
    commit();selectedLayer=juce::jlimit(0,1,layer);page=0;selectedStep=0;playhead=-1;observedRevision=~uint64_t{};refresh();
}
void SequenceCanvas::resizePattern(){
    if(length.getText().isEmpty()){controls();return;}
    const int count=juce::jlimit(0,maxFrames,length.getText().getIntValue());
    length.setText(juce::String(count),false);
    if(count==(int)data.values.size())return;
    data.values.resize((size_t)count,(int16_t)(selectedLane==0?15:0));
    if(data.releaseStart>=count)data.releaseStart=-1;
    const int boundary=data.releaseStart>=0?data.releaseStart:count;
    if(data.loopStart>=boundary)data.loopStart=boundary>0?boundary-1:-1;
    if(count==0){data.loopStart=-1;data.releaseStart=-1;}
    selectedStep=juce::jlimit(0,juce::jmax(0,count-1),selectedStep);dirty=true;commit();
}
void SequenceCanvas::commit(){
    if(!dirty)return;
    const auto error=processor.setSequenceText(selectedLane,SequenceSnapshot::toText(data),selectedLayer);
    if(error.isNotEmpty())return;
    dirty=false;data=processor.sequenceData(selectedLane,selectedLayer);observedRevision=data.revision;
    // Drawing is a deliberate instruction to hear this lane.
    const char* ids[]={"isVolumeSequenceEnabled_raw","isPitchSequenceEnabled_raw","isDutySequenceEnabled_raw"};
    if(!data.values.empty())if(auto* p=processor.state.getParameter(RebitProcessor::layerParameter(ids[selectedLane],selectedLayer))){
        if(p->getValue()<.5f){p->beginChangeGesture();p->setValueNotifyingHost(1.f);p->endChangeGesture();}
    }
    controls();repaint();if(onCommitted)onCommitted(selectedLane);
}
void SequenceCanvas::refresh(){
    if(processor.sequenceRevision(selectedLayer)!=observedRevision&&!dirty&&!drawing&&dragMarker==0){
        data=processor.sequenceData(selectedLane,selectedLayer);observedRevision=data.revision;
        if(selectedLane==1){
            int extent=0;for(auto v:data.values)extent=juce::jmax(extent,std::abs((int)v));
            if(extent>pitchRange)pitchRange=extent>24?64:24;
        }
        selectedStep=juce::jlimit(0,juce::jmax(0,(int)data.values.size()-1),selectedStep);controls();repaint();
    }
    const int position=processor.sequencePosition(selectedLane,selectedLayer);
    if(position!=playhead){playhead=position;repaint();}
}
void SequenceCanvas::timerCallback(){commit();refresh();}
void SequenceCanvas::drawAt(juce::Point<float> point){
    seed();const int step=stepAt(point.x);if(step>=(int)data.values.size())return;
    auto bars=grid().withTrimmedTop(3).withTrimmedBottom(18);
    const float unit=juce::jlimit(0.f,1.f,(bars.getBottom()-point.y)/bars.getHeight());
    const int value=(int)std::round((float)minValue()+unit*(float)(maxValue()-minValue()));
    const int start=lastStep<0?step:lastStep, end=step;
    const int previousValue=(int)data.values[(size_t)start];
    for(int i=juce::jmin(start,end);i<=juce::jmax(start,end);++i){
        const float t=start==end?1.f:(float)(i-start)/(float)(end-start);
        data.values[(size_t)i]=(int16_t)std::round((float)previousValue+t*(float)(value-previousValue));
    }
    lastStep=step;selectedStep=step;dirty=true;controls();repaint();
}
void SequenceCanvas::mouseDown(const juce::MouseEvent& e){
    grabKeyboardFocus();
    if(e.y>=32&&e.y<50&&!data.values.empty()){
        const int boundary=data.releaseStart>=0?data.releaseStart:(int)data.values.size();
        if(data.loopStart>=0&&juce::Rectangle<float>(juce::jlimit(0.f,(float)getWidth()-40.f,stepX(data.loopStart)),32,40,18).contains(e.position))dragMarker=1;
        else if(data.releaseStart>=0&&juce::Rectangle<float>(juce::jlimit(0.f,(float)getWidth()-40.f,stepX(boundary)),32,40,18).contains(e.position))dragMarker=2;
        if(dragMarker!=0)return;
    }
    if(grid().contains(e.position)){drawing=true;lastStep=-1;drawAt(e.position);}
}
void SequenceCanvas::mouseDrag(const juce::MouseEvent& e){
    if(dragMarker!=0){
        int position=page*pageSize+(int)std::round(e.position.x/grid().getWidth()*(float)pageSize);
        if(dragMarker==1)data.loopStart=juce::jlimit(0,(data.releaseStart>=0?data.releaseStart:(int)data.values.size())-1,position);
        else{
            data.releaseStart=juce::jlimit(1,(int)data.values.size()-1,position);
            if(data.loopStart>=data.releaseStart)data.loopStart=data.releaseStart-1;
        }
        dirty=true;controls();repaint();
    }else if(drawing)drawAt(e.position);
}
void SequenceCanvas::mouseUp(const juce::MouseEvent&){drawing=false;dragMarker=0;lastStep=-1;commit();}
bool SequenceCanvas::keyPressed(const juce::KeyPress& key){
    const int code=key.getKeyCode();
    if(key.getModifiers().isCommandDown()||key.getModifiers().isAltDown())return false;
    if(code==juce::KeyPress::leftKey||code==juce::KeyPress::rightKey||code==juce::KeyPress::homeKey||code==juce::KeyPress::endKey){
        if(data.values.empty())return true;
        selectedStep=code==juce::KeyPress::homeKey?0:code==juce::KeyPress::endKey?(int)data.values.size()-1:juce::jlimit(0,(int)data.values.size()-1,selectedStep+(code==juce::KeyPress::leftKey?-1:1));
        page=selectedStep/pageSize;controls();repaint();return true;
    }
    if(code=='L'||code=='R'){
        seed();
        if(code=='L')data.loopStart=juce::jmin(selectedStep,(data.releaseStart>=0?data.releaseStart:(int)data.values.size())-1);
        else if(selectedStep>0){data.releaseStart=selectedStep;if(data.loopStart>=selectedStep)data.loopStart=selectedStep-1;}
        dirty=true;commit();return true;
    }
    if(code==juce::KeyPress::upKey||code==juce::KeyPress::downKey){
        seed();const int lo=selectedLane==1?-64:0,hi=selectedLane==0?15:selectedLane==1?63:2;
        auto& value=data.values[(size_t)selectedStep];value=(int16_t)juce::jlimit(lo,hi,(int)value+(code==juce::KeyPress::upKey?1:-1));
        if(selectedLane==1&&std::abs((int)value)>pitchRange)pitchRange=std::abs((int)value)>24?64:24;
        dirty=true;commit();return true;
    }
    return false;
}
void SequenceCanvas::resized(){
    for(int i=0;i<3;++i)tabs[(size_t)i].setBounds(i*82,0,76,27);
    stepsLabel.setBounds(262,0,45,27);length.setBounds(310,0,58,27);
    loop.setBounds(386,0,65,27);release.setBounds(462,0,82,27);range.setBounds(556,0,64,27);
    valueLabel.setBounds(634,0,174,27);
    previous.setBounds(getWidth()-212,0,26,27);pageLabel.setBounds(getWidth()-180,0,148,27);next.setBounds(getWidth()-26,0,26,27);
}
void SequenceCanvas::paint(juce::Graphics& g){
    const auto bounds=grid();const float cell=bounds.getWidth()/(float)pageSize;
    const auto bars=bounds.withTrimmedTop(3).withTrimmedBottom(18);
    const auto colour=accent();g.setColour(inset);g.fillRoundedRectangle(bounds,4);
    const float rangeSize=(float)(maxValue()-minValue());
    const auto valueY=[&](int value){return bars.getBottom()-juce::jlimit(0.f,1.f,(float)(value-minValue())/rangeSize)*bars.getHeight();};
    const float zero=valueY(0);
    for(int row=0;row<=4;++row){g.setColour(line.withAlpha(.5f));g.drawHorizontalLine((int)(bars.getY()+bars.getHeight()*(float)row/4),0,bounds.getWidth());}
    if(selectedLane==1){g.setColour(colour.withAlpha(.3f));g.drawHorizontalLine((int)zero,0,bounds.getWidth());}
    const int boundary=data.releaseStart>=0?data.releaseStart:(int)data.values.size();
    for(int col=0;col<pageSize;++col){
        const int step=page*pageSize+col;const float x=(float)col*cell;
        g.setColour(line.withAlpha(col%4==0?.85f:.4f));g.drawVerticalLine((int)x,bounds.getY(),bounds.getBottom());
        if(step<(int)data.values.size()){
            const bool tail=data.releaseStart>=0&&step>=data.releaseStart;
            const int value=(int)data.values[(size_t)step];const float y=valueY(value);
            const float top=selectedLane==1?juce::jmin(zero,y):y;
            const float bottom=selectedLane==1?juce::jmax(zero,y):bars.getBottom();
            g.setColour(colour.withAlpha(tail?.13f:.23f));g.fillRect(x+3,top,cell-6,juce::jmax(2.f,bottom-top));
            g.setColour(colour.withAlpha(tail?.5f:.95f));g.fillRect(x+3,y,cell-6,2.f);
            g.setColour(ink.withAlpha(.9f));g.setFont(mono(9));
            g.drawText(juce::String(value),juce::Rectangle<float>(x,bars.getBottom()+3,cell,14),juce::Justification::centred);
            if(step==playhead){g.setColour(colour.withAlpha(.12f));g.fillRect(x,bars.getY(),cell,bars.getHeight());g.setColour(ink);g.fillRect(x+cell*.5f-2,bars.getY()+3,4.f,4.f);}
            if(step==selectedStep&&hasKeyboardFocus(true)){g.setColour(colour.withAlpha(.65f));g.drawRect(juce::Rectangle<float>(x+1,bounds.getY()+1,cell-2,bounds.getHeight()-2),1);}
        }else{g.setColour(muted.withAlpha(.2f));g.fillRect(x+cell*.5f-1,bars.getCentreY(),2.f,2.f);}
    }
    if(data.values.empty()){
        g.setColour(inset.withAlpha(.8f));g.fillRect(bounds.reduced(4));
        g.setColour(muted);g.setFont(mono(11));g.drawText("DRAW TO CREATE A 16-STEP PATTERN",bounds,juce::Justification::centred);
    }
    auto marker=[&](int step,const char* name,juce::Colour c){
        if(step<page*pageSize||step>(page+1)*pageSize)return;
        const float x=juce::jlimit(0.f,(float)getWidth()-40.f,stepX(step));
        g.setColour(c);g.fillRoundedRectangle(x,32,40,16,2);
        g.setColour(inset);g.setFont(mono(9));g.drawText(name,juce::Rectangle<float>(x,32,40,16),juce::Justification::centred);
        g.setColour(c.withAlpha(.5f));g.drawVerticalLine((int)stepX(step),48,bounds.getBottom());
    };
    if(data.loopStart>=0)marker(data.loopStart,"LOOP",colour);
    if(data.releaseStart>=0)marker(boundary,"REL",juce::Colour(0xffffd23f));
}
