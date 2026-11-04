#include "PluginProcessor.h"
#include "PluginEditor.h"

juce::AudioProcessorValueTreeState::ParameterLayout RebitProcessor::layout() {
    juce::AudioProcessorValueTreeState::ParameterLayout l;
    auto choice=[&](const char* id,const char* name,juce::StringArray items,int d=0){l.add(std::make_unique<juce::AudioParameterChoice>(juce::ParameterID(id,1),name,items,d));};
    auto number=[&](const char* id,const char* name,float lo,float hi,float d,float skew=1.f,float interval=0.f){juce::NormalisableRange<float> r(lo,hi,interval);r.skew=skew;l.add(std::make_unique<juce::AudioParameterFloat>(juce::ParameterID(id,1),name,r,d));};
    auto boolean=[&](const char* id,const char* name,bool d=false){l.add(std::make_unique<juce::AudioParameterBool>(juce::ParameterID(id,1),name,d));};
    choice("osc","OSC Type",{"Pulse/Square","Triangle","Noise"});
    choice("duty","Duty",{"12.5%","25%","50%"});
    choice("noiseAlgorithm_raw","Noise algorithm",{"4-bit random","1-bit long cycle","1-bit short cycle"});
    number("gain","Gain",0,1,.5f,1.f,.01f);
    l.add(std::make_unique<juce::AudioParameterInt>(juce::ParameterID("maxPoly",1),"Max Poly",1,64,8));
    number("attack","Attack",0,5,0,.5f,.001f);number("decay","Decay",0,5,0,.5f,.001f);number("suslevel","Sustain",0,1,1,1.f,.01f);number("release","Release",0,5,0,.5f,.001f);
    choice("monophonicBehavior_raw","Behavior",{"Legato","Arpeggio Up","Arpeggio Down","Non-legato"});
    choice("arpeggioIntervalType_raw","Arpeggio interval",{"1 frame","2 frames","3 frames","96th","64th","48th","32nd","24th","Slider"});
    number("arpeggioIntervalSliderValue","Arpeggio seconds",.001f,.3f,.1f,.5f,.001f);
    number("portamentoTime","Portamento Time",0,1,0,1.f,.01f);number("bendRange","Bend Range",0,24,2,1.f,1.f);
    number("vibratoRate","Vibrato period",.01f,1,.15f,.5f,.001f);number("vibratoDepth","Vibrato depth",0,2,0,1.f,.01f);number("vibratoDelay","Vibrato delay",0,1,.3f,1.f,.01f);
    boolean("vibratoIgnoresWheel_raw","Ignores mod wheel",true);
    number("sweepInitialPitch","Initial sweep pitch",-24,24,0,1.f,1.f);number("sweepTime","Sweep time",.01f,5,.1f,.5f,.001f);
    boolean("isVolumeSequenceEnabled_raw","Volume sequence");boolean("isPitchSequenceEnabled_raw","Pitch sequence");boolean("isDutySequenceEnabled_raw","Duty sequence");
    choice("pitchSequenceMode_raw","Pitch sequence mode",{"Coarse","Fine"});
    boolean("arpEnabled","Arpeggiator");boolean("arpLatch","Arp latch");
    choice("arpPattern","Arp pattern",{"Up","Down","Up / Down","Random","Played order"});
    choice("arpRate","Arp rate",{"1/4","1/8","1/16","1/32","1/64","1/4 dotted","1/8 dotted","1/16 dotted","1/32 dotted","1/64 dotted","1/4 triplet","1/8 triplet","1/16 triplet","1/32 triplet","1/64 triplet","Seconds"},2);
    choice("arpOctaves","Arp octaves",{"1","2","3","4"});
    choice("arpArticulation","Arp articulation",{"Retrigger","Legato"});
    number("arpSeconds","Arp step seconds",.01f,2.f,.125f,.5f,.001f);
    number("arpGate","Arp gate",5,100,65,1.f,1.f);number("arpSwing","Arp swing",0,49,0,1.f,1.f);
    boolean("hostSync","Host sync",true);
    choice("sequenceClock","Sequence clock",{"60 Hz","1/4","1/8","1/16","1/32","1/64","1/8 triplet","1/16 triplet"});
    return l;
}
RebitProcessor::RebitProcessor()
    : AudioProcessor(BusesProperties().withOutput("Output",juce::AudioChannelSet::stereo(),true)),
      state(*this,nullptr,"RebitRustParams",layout()), engine(rebit_engine_new()), canonical(rebit_bank_new()) {}
RebitProcessor::~RebitProcessor() {
    // Hosts stop processing before destruction. No callback owns these handles now.
    rebit_engine_free(engine);rebit_bank_free(canonical);rebit_bank_free(pending);
}
float RebitProcessor::value(const char* id) const {auto* v=state.getRawParameterValue(id);return v?v->load():0;}
void RebitProcessor::readParameters(bool readTransport) {
    RustParameters p{};
    p.osc=(int)value("osc");p.duty=(int)value("duty");p.noise_algorithm=(int)value("noiseAlgorithm_raw");p.max_poly=(int)value("maxPoly");p.behavior=(int)value("monophonicBehavior_raw");
    p.gain=value("gain");p.attack=value("attack");p.decay=value("decay");p.sustain=value("suslevel");p.release=value("release");
    p.portamento=value("portamentoTime");p.bend_range=value("bendRange");p.vibrato_period=value("vibratoRate");p.vibrato_depth=value("vibratoDepth");p.vibrato_delay=value("vibratoDelay");p.ignores_wheel=value("vibratoIgnoresWheel_raw")>.5f;
    p.sweep_initial=value("sweepInitialPitch");p.sweep_time=value("sweepTime");p.interval=value("arpeggioIntervalSliderValue");p.interval_type=(int)value("arpeggioIntervalType_raw");
    p.volume_enabled=value("isVolumeSequenceEnabled_raw")>.5f;p.pitch_enabled=value("isPitchSequenceEnabled_raw")>.5f;p.duty_enabled=value("isDutySequenceEnabled_raw")>.5f;p.pitch_mode=(int)value("pitchSequenceMode_raw");
    RustTransportInfo transport{};transport.bpm=120.;bool tempoValid=false;
    if(readTransport)if(auto* ph=getPlayHead())if(auto pos=ph->getPosition()){
        if(auto tempo=pos->getBpm())if(std::isfinite(*tempo)&&*tempo>0.){transport.bpm=*tempo;tempoValid=true;}
        if(auto ppq=pos->getPpqPosition())if(std::isfinite(*ppq)){transport.ppq=*ppq;transport.valid=tempoValid?1:0;}
        transport.playing=pos->getIsPlaying()?1:0;
        if(auto loop=pos->getLoopPoints()){
            transport.loop_start=loop->ppqStart;transport.loop_end=loop->ppqEnd;transport.looping=pos->getIsLooping()?1:0;
        }
    }
    RustArpParameters arp{};
    arp.enabled=value("arpEnabled")>.5f;arp.pattern=(int)value("arpPattern");arp.rate=(int)value("arpRate");
    arp.octaves=(int)value("arpOctaves")+1;arp.latch=value("arpLatch")>.5f;arp.retrigger=1-(int)value("arpArticulation");
    arp.seconds=value("arpSeconds");arp.gate=value("arpGate")*.01f;arp.swing=value("arpSwing")*.01f;
    rebit_engine_arp_parameters(engine,&arp);
    rebit_engine_parameters(engine,&p,transport.bpm);
    RustClockParameters clocks{value("hostSync")>.5f?1:0,(int)value("sequenceClock")};
    rebit_engine_clock_parameters(engine,&clocks);
    rebit_engine_transport(engine,&transport);
}
void RebitProcessor::prepareToPlay(double sr,int) {rebit_engine_prepare(engine,sr);readParameters(false);keyboard.reset();}
bool RebitProcessor::isBusesLayoutSupported(const BusesLayout& l) const {return l.getMainInputChannelSet().isDisabled()&&(l.getMainOutputChannelSet()==juce::AudioChannelSet::mono()||l.getMainOutputChannelSet()==juce::AudioChannelSet::stereo());}
void RebitProcessor::midi(const juce::MidiMessage& m) {
    const auto* data=m.getRawData();const int n=m.getRawDataSize();
    if(n>0)rebit_engine_midi(engine,data[0],n>1?data[1]:0,n>2?data[2]:0);
}
void RebitProcessor::processBlock(juce::AudioBuffer<float>& buffer,juce::MidiBuffer& messages) {
    juce::ScopedNoDenormals noDenormals;
    if(resetRequested.exchange(false))rebit_engine_reset(engine);
    readParameters();
    {
        const juce::SpinLock::ScopedTryLockType lock(sequenceLock);
        if(lock.isLocked()&&sequenceDirty){
            pending=rebit_engine_swap_bank(engine,pending);
            sequenceDirty=false;
        }
    }
    keyboard.processNextMidiBuffer(messages,0,buffer.getNumSamples(),true);
    buffer.clear();
    int at=0;
    auto render=[&](int end){
        end=std::clamp(end,at,buffer.getNumSamples());
        if(end<=at||buffer.getNumChannels()<=0)return;
        const int n=end-at;
        float* first=buffer.getWritePointer(0)+at;
        rebit_engine_render(engine,first,(size_t)n);
        for(int c=1;c<buffer.getNumChannels();++c)std::copy(first,first+n,buffer.getWritePointer(c)+at);
        at=end;
    };
    for(const auto metadata:messages){render(metadata.samplePosition);midi(metadata.getMessage());}
    render(buffer.getNumSamples());messages.clear();
    clockState.store(rebit_engine_clock_status(engine),std::memory_order_relaxed);
    int32_t positions[3];rebit_engine_positions(engine,positions);
    for(size_t lane=0;lane<3;++lane)playheads[lane].store(positions[lane],std::memory_order_relaxed);
}
juce::String SequenceSnapshot::toText(const SequenceSnapshot& data) {
    if(data.values.empty())return {};
    const int count=(int)data.values.size();
    const int boundary=data.releaseStart>=0?data.releaseStart:count;
    if(boundary<1||boundary>count||data.loopStart>=boundary||data.loopStart < -1||data.releaseStart < -1)return {};
    auto join=[&](int from,int to){juce::String result;for(int i=from;i<to;++i){if(i>from)result+=",";result+=juce::String((int)data.values[(size_t)i]);}return result;};
    auto text=data.loopStart>=0?join(0,data.loopStart)+"["+join(data.loopStart,boundary)+"]":join(0,boundary);
    if(data.releaseStart>=0&&data.releaseStart<count)text+="|"+join(data.releaseStart,count);
    return text;
}
SequenceSnapshot RebitProcessor::sequenceData(int lane) const {
    SequenceSnapshot result;
    if(lane<0||lane>=3)return result;
    const juce::ScopedLock lock(textMutex);
    RustSequenceInfo info{};
    if(!rebit_bank_read(canonical,(size_t)lane,nullptr,0,&info))return result;
    result.values.resize(info.count);
    rebit_bank_read(canonical,(size_t)lane,result.values.data(),result.values.size(),&info);
    result.loopStart=info.loop_start;result.releaseStart=info.release_start;result.revision=sequenceVersion.load();return result;
}
juce::String RebitProcessor::sequenceText(int i) const {const juce::ScopedLock lock(textMutex);return i>=0&&i<3?texts[(size_t)i]:juce::String{};}
juce::String RebitProcessor::setSequenceText(int i,const juce::String& text) {
    if(i<0||i>2)return "Unknown sequence";
    const juce::ScopedLock textLock(textMutex);
    auto* next=rebit_bank_clone(canonical);
    uint8_t error[256]{};
    const auto utf8=text.toUTF8();
    if(!rebit_bank_set(next,(size_t)i,reinterpret_cast<const uint8_t*>(utf8.getAddress()),text.getNumBytesAsUTF8(),error,sizeof(error))){
        rebit_bank_free(next);return juce::String::fromUTF8(reinterpret_cast<const char*>(error));
    }
    rebit_bank_free(canonical);canonical=next;
    auto* publication=rebit_bank_clone(canonical);
    {
        const juce::SpinLock::ScopedLockType lock(sequenceLock);
        std::swap(pending,publication);sequenceDirty=true;
    }
    // The old publication (or retired audio bank) is destroyed only here.
    rebit_bank_free(publication);
    texts[(size_t)i]=text;++sequenceVersion;
    return {};
}
void RebitProcessor::getStateInformation(juce::MemoryBlock& out) {
    const juce::ScopedLock lock(textMutex);
    auto xml=std::make_unique<juce::XmlElement>("RebitRust");xml->setAttribute("version",1);
    xml->addChildElement(state.copyState().createXml().release());
    const char* names[]={"volume","pitch","duty"};
    for(int i=0;i<3;++i)xml->createNewChildElement(names[i])->addTextElement(texts[(size_t)i]);
    copyXmlToBinary(*xml,out);
}
void RebitProcessor::setStateInformation(const void* data,int size) {
    auto xml=getXmlFromBinary(data,size);
    if(!xml||!xml->hasTagName("RebitRust")||xml->getIntAttribute("version")!=1)return;
    auto* params=xml->getChildByName("RebitRustParams");
    if(!params)return;
    const juce::ScopedLock textLock(textMutex);
    auto* next=rebit_bank_new();
    std::array<juce::String,3> restored;
    const char* names[]={"volume","pitch","duty"};
    for(size_t i=0;i<3;++i){
        auto* child=xml->getChildByName(names[i]);restored[i]=child?child->getAllSubText():juce::String{};
        const auto utf8=restored[i].toUTF8();uint8_t error[256]{};
        if(!rebit_bank_set(next,i,reinterpret_cast<const uint8_t*>(utf8.getAddress()),restored[i].getNumBytesAsUTF8(),error,sizeof(error))){
            rebit_bank_free(next);return;
        }
    }
    auto parameterTree=juce::ValueTree::fromXml(*params);
    // Earlier version-1 patches have no independent-arp fields. Materialize the
    // new defaults explicitly instead of inheriting the currently loaded arp.
    const char* arpIds[]={"arpEnabled","arpLatch","arpPattern","arpRate","arpOctaves","arpArticulation","arpSeconds","arpGate","arpSwing"};
    for(auto* id:arpIds)if(!parameterTree.getChildWithProperty("id",id).isValid()){
        if(auto* p=state.getParameter(id)){
            juce::ValueTree child("PARAM");child.setProperty("id",id,nullptr);
            child.setProperty("value",p->convertFrom0to1(p->getDefaultValue()),nullptr);parameterTree.addChild(child,-1,nullptr);
        }
    }
    if(!parameterTree.getChildWithProperty("id","hostSync").isValid()){
        // Preserve chord-start timing in patches written before transport sync.
        juce::ValueTree child("PARAM");child.setProperty("id","hostSync",nullptr);child.setProperty("value",0.f,nullptr);parameterTree.addChild(child,-1,nullptr);
    }
    if(!parameterTree.getChildWithProperty("id","sequenceClock").isValid()){
        juce::ValueTree child("PARAM");child.setProperty("id","sequenceClock",nullptr);child.setProperty("value",0.f,nullptr);parameterTree.addChild(child,-1,nullptr);
    }
    state.replaceState(parameterTree);
    rebit_bank_free(canonical);canonical=next;texts=restored;
    auto* publication=rebit_bank_clone(canonical);
    {
        const juce::SpinLock::ScopedLockType lock(sequenceLock);
        std::swap(pending,publication);sequenceDirty=true;
    }
    rebit_bank_free(publication);
    ++sequenceVersion;resetRequested=true;
}
juce::AudioProcessorEditor* RebitProcessor::createEditor(){return new RebitEditor(*this);}
juce::AudioProcessor* JUCE_CALLTYPE createPluginFilter(){return new RebitProcessor();}
