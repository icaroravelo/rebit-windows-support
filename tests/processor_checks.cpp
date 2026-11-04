#include "PluginProcessor.h"
#include "SequenceCanvas.h"
#include "TestPlayHead.h"
#include <atomic>
#include <cmath>
#include <iostream>
#include <thread>
#include <stdexcept>
static void require(bool condition,const char* message) {if(!condition)throw std::runtime_error(message);}
int main() {
    try {
        juce::ScopedJuceInitialiser_GUI init;
        {
            RebitProcessor graphical;
            graphical.setSequenceText(0,"[15x12]|3,2,1,0");
            SequenceCanvas canvas(graphical);canvas.setSize(1032,148);
            auto event=[&](juce::Point<float> point){return juce::MouseEvent(
                juce::Desktop::getInstance().getMainMouseSource(),point,juce::ModifierKeys::leftButtonModifier,
                1.f,0.f,0.f,0.f,0.f,&canvas,&canvas,juce::Time::getCurrentTime(),point,juce::Time::getCurrentTime(),1,true);};
            canvas.mouseDown(event({14,38}));canvas.mouseDrag(event({202,38}));canvas.mouseUp(event({202,38}));
            require(graphical.sequenceData(0).loopStart==3,"Loop marker gesture did not update patch");
            canvas.mouseDown(event({785,38}));canvas.mouseDrag(event({650,38}));canvas.mouseUp(event({650,38}));
            require(graphical.sequenceData(0).releaseStart==10,"Release marker gesture did not update patch");
            canvas.mouseDown(event({32,100}));canvas.mouseUp(event({32,100}));
            require(graphical.sequenceData(0).values[0]==6,"Drawing did not update the engine sequence");
            require(graphical.state.getRawParameterValue("isVolumeSequenceEnabled_raw")->load()>.5f,"Drawing did not enable the lane");
            canvas.keyPressed(juce::KeyPress(juce::KeyPress::upKey));
            require(graphical.sequenceData(0).values[0]==7,"Keyboard fine adjustment failed");
            graphical.setSequenceText(1,"-64,0,63");canvas.selectLane(1);
            canvas.mouseDown(event({100,53}));canvas.mouseUp(event({100,53}));
            const auto pitch=graphical.sequenceData(1);
            require(pitch.values[0]==-64&&pitch.values[1]==63&&pitch.values[2]==63,"Full-range pitch editing corrupted a step");
        }
        {
            RebitProcessor arp;
            auto set=[&](const char* id,float value){auto* p=arp.state.getParameter(id);require(p!=nullptr,"Missing arp parameter");p->setValueNotifyingHost(p->convertTo0to1(value));};
            set("arpEnabled",1);set("arpRate",15);set("arpSeconds",.01f);set("arpGate",50);
            arp.prepareToPlay(48000,512);juce::AudioBuffer<float> block(2,512);juce::MidiBuffer events;
            events.addEvent(juce::MidiMessage::noteOn(1,60,(juce::uint8)127),17);
            events.addEvent(juce::MidiMessage::noteOn(1,67,(juce::uint8)127),17);arp.processBlock(block,events);
            for(int i=0;i<512;++i){
                const bool on=(i>=17&&i<257)||i>=497;
                require(on?std::abs(block.getSample(0,i))>.49f:block.getSample(0,i)==0.f,"Independent arp gate/sample offset differs");
            }
            juce::MemoryBlock modern;arp.getStateInformation(modern);set("arpEnabled",0);set("arpGate",10);
            arp.setStateInformation(modern.getData(),(int)modern.getSize());
            require(arp.state.getRawParameterValue("arpEnabled")->load()>.5f&&arp.state.getRawParameterValue("arpGate")->load()==50.f,"New arp state recall failed");
            auto tree=arp.state.copyState();
            for(int i=tree.getNumChildren();--i>=0;)if(tree.getChild(i).getProperty("id").toString().startsWith("arp")||tree.getChild(i).getProperty("id").toString()=="hostSync"||tree.getChild(i).getProperty("id").toString()=="sequenceClock")tree.removeChild(i,nullptr);
            tree.getChildWithProperty("id","monophonicBehavior_raw").setProperty("value",1,nullptr);
            tree.getChildWithProperty("id","maxPoly").setProperty("value",1,nullptr);
            juce::XmlElement root("RebitRust");root.setAttribute("version",1);root.addChildElement(tree.createXml().release());
            juce::MemoryBlock legacy;juce::AudioProcessor::copyXmlToBinary(root,legacy);
            arp.setStateInformation(legacy.getData(),(int)legacy.getSize());
            require(arp.state.getRawParameterValue("hostSync")->load()<.5f&&arp.state.getRawParameterValue("sequenceClock")->load()<.5f,"Old patch did not retain free/60-Hz clocks");
            require(arp.state.getRawParameterValue("arpEnabled")->load()<.5f,"Old patch inherited enabled independent arp");
            require(arp.state.getRawParameterValue("arpGate")->load()==65.f&&arp.state.getRawParameterValue("arpRate")->load()==2.f,"Old patch did not receive arp defaults");
            require(arp.state.getRawParameterValue("monophonicBehavior_raw")->load()==1.f,"Old mono arp behavior changed");
            set("arpEnabled",1);set("arpLatch",1);arp.processBlock(block,events);
            events.addEvent(juce::MidiMessage::noteOn(1,60,(juce::uint8)127),0);arp.processBlock(block,events);
            events.addEvent(juce::MidiMessage::noteOff(1,60),0);arp.processBlock(block,events);
            arp.stopArpeggiator();arp.processBlock(block,events);require(block.getMagnitude(0,512)==0.f,"STOP did not clear the latched arp");
        }
        {
            TestPlayHead host;host.set(.1,120.,true);RebitProcessor synced;synced.setPlayHead(&host);
            auto set=[&](const char* id,float value){auto* p=synced.state.getParameter(id);require(p!=nullptr,"Missing clock parameter");p->setValueNotifyingHost(p->convertTo0to1(value));};
            set("arpEnabled",1);set("hostSync",1);set("arpGate",50);synced.prepareToPlay(8000,2400);
            juce::AudioBuffer<float> audio(2,1200);juce::MidiBuffer events;
            events.addEvent(juce::MidiMessage::noteOn(1,60,(juce::uint8)127),17);
            events.addEvent(juce::MidiMessage::noteOn(1,67,(juce::uint8)127),17);synced.processBlock(audio,events);
            for(int i=0;i<1200;++i)require(i>=600&&i<1100?std::abs(audio.getSample(0,i))>.49f:audio.getSample(0,i)==0.f,"JUCE host grid/gate alignment differs");
            require(synced.clockStatus()==2,"Host play status missing");
            host.set(.4,120.,false);synced.processBlock(audio,events);require(audio.getMagnitude(0,1200)==0.f&&synced.clockStatus()==1,"Host stop did not suspend arp");
            host.set(4.,120.,true);synced.processBlock(audio,events);require(std::abs(audio.getSample(0,0)+.5f)<1e-7f,"Host restart lost held chord");
            host.set(0.,120.,true,true,0.,.5);audio.setSize(2,2400);synced.processBlock(audio,events);
            for(int i=0;i<400;++i)require(std::abs(audio.getSample(0,i)-audio.getSample(0,2000+i))<1e-7f,"In-block cycle did not restart pattern");
            host.set(.1,240.,true);audio.setSize(2,600);synced.processBlock(audio,events);
            for(int i=50;i<300;++i)require(audio.getSample(0,i)==0.f,"Tempo change moved closed gate");
            require(std::abs(audio.getSample(0,300))>.49f,"Tempo change missed next grid");
            set("hostSync",0);host.set(.4,240.,false);synced.processBlock(audio,events);
            require(audio.getMagnitude(0,600)>.49f&&synced.clockStatus()==0,"Free mode failed during host stop");
            set("hostSync",1);host.available=false;synced.processBlock(audio,events);
            require(audio.getMagnitude(0,600)>.49f&&synced.clockStatus()==3,"Missing playhead did not fall back to free clock");
            set("sequenceClock",3);juce::MemoryBlock state;synced.getStateInformation(state);set("sequenceClock",0);synced.setStateInformation(state.getData(),(int)state.getSize());
            require(std::abs(synced.state.getRawParameterValue("sequenceClock")->load()-3.f)<.01f,"Sequence clock state did not round-trip");
        }
        RebitProcessor p;
        p.prepareToPlay(48000,128);
        juce::AudioBuffer<float> buffer(2,128);
        juce::MidiBuffer midi;
        midi.addEvent(juce::MidiMessage::noteOn(1,69,(juce::uint8)127),17);
        midi.addEvent(juce::MidiMessage::noteOff(1,69),83);
        p.processBlock(buffer,midi);
        for(int i=0;i<128;++i){
            const float v=buffer.getSample(0,i);
            require(std::isfinite(v)&&std::abs(v-buffer.getSample(1,i))<1e-8f,"Invalid stereo output");
            require((i>=17&&i<83)?std::abs(v)>.49f:v==0.f,"MIDI event sample offset mismatch");
        }
        for(const auto* text:{"", "15,12,9", "[15,12,9]", "15[12,9]|3,0", "15,12|9,3,0"}){
            require(p.setSequenceText(0,text).isEmpty(),"Snapshot source rejected");
            const auto snapshot=p.sequenceData(0);
            const auto canonical=SequenceSnapshot::toText(snapshot);
            require(p.setSequenceText(0,canonical).isEmpty(),"Graphical serialization rejected");
            const auto roundtrip=p.sequenceData(0);
            require(roundtrip.values==snapshot.values&&roundtrip.loopStart==snapshot.loopStart&&roundtrip.releaseStart==snapshot.releaseStart,"Graphical/text roundtrip differs");
        }
        p.setSequenceText(0,"[15,14,13,12,11,10,9,8]");
        p.state.getParameter("isVolumeSequenceEnabled_raw")->setValueNotifyingHost(1.f);
        midi.addEvent(juce::MidiMessage::noteOn(1,60,(juce::uint8)127),0);
        for(int i=0;i<7;++i)p.processBlock(buffer,midi);
        require(p.sequencePosition(0)==1,"Audio playhead telemetry did not advance");
        p.setSequenceText(0,"[15,13,11,9,7,5,3,1]");p.processBlock(buffer,midi);
        require(p.sequencePosition(0)==1,"Live sequence edit restarted playback");
        p.state.getParameter("isVolumeSequenceEnabled_raw")->setValueNotifyingHost(0.f);
        midi.addEvent(juce::MidiMessage::allSoundOff(1),0);p.processBlock(buffer,midi);
        require(p.setSequenceText(0,"15[10,5]|3,0").isEmpty(),"Valid sequence rejected");
        require(p.setSequenceText(1,"0,7|12").isEmpty(),"Pitch sequence rejected");
        require(p.setSequenceText(0,"15x99999999").isNotEmpty(),"Invalid sequence accepted");
        require(p.sequenceText(0)=="15[10,5]|3,0","Invalid edit replaced last valid text");
        juce::MemoryBlock saved;p.getStateInformation(saved);
        p.setSequenceText(0,"");p.setSequenceText(1,"");
        p.setStateInformation(saved.getData(),(int)saved.getSize());
        require(p.sequenceText(0)=="15[10,5]|3,0"&&p.sequenceText(1)=="0,7|12","State did not restore sequences");
        p.setSequenceText(0,"");p.setSequenceText(1,"");
        juce::MemoryBlock empty;p.getStateInformation(empty);
        p.setSequenceText(0,"15");p.setStateInformation(empty.getData(),(int)empty.getSize());
        require(p.sequenceText(0).isEmpty(),"Empty saved sequence did not clear current data");
        midi.addEvent(juce::MidiMessage::noteOn(1,60,(juce::uint8)127),0);
        std::atomic<bool> running{true};
        std::thread writer([&]{for(int i=0;i<1000;++i)p.setSequenceText(i%3,i%3==1?"0[7,12]|0":i%3==0?"15[10,5]|0":"0[1,2]|0");running=false;});
        while(running.load()){
            p.processBlock(buffer,midi);
            for(int i=0;i<128;++i)require(std::isfinite(buffer.getSample(0,i)),"Nonfinite output during sequence handoff");
        }
        writer.join();p.processBlock(buffer,midi);p.releaseResources();
        std::cout<<"Processor: MIDI offsets, stereo, parser errors, state, empty clearing, concurrent sequence handoff passed\n";
    }catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}
}
