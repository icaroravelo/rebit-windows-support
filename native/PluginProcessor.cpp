#include "PluginProcessor.h"
#include "PluginEditor.h"
#include "Presets.h"
#include <array>

namespace {
struct DeterministicRandom {
    uint32_t state;
    explicit DeterministicRandom (uint32_t seed) : state (seed ? seed : 0x9e3779b9u) {}
    uint32_t next() {
        uint32_t x = state;
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        state = x;
        return x;
    }
    float unit() { return (float) (next() >> 8) * (1.f / 16777216.f); }
    int range (int lo, int hi) { return lo + (int) (next() % (uint32_t) (hi - lo + 1)); }
    float range (float lo, float hi) { return lo + (hi - lo) * unit(); }
    bool chance (float probability) { return unit() < probability; }
};
enum RandomSection : int {
    RandomOsc = 1, RandomEnv = 2, RandomSeq = 4,
    RandomMod = 8, RandomFx = 16, RandomMix = 32, RandomAll = 63
};
juce::String join (const juce::Array<int>& values) {
    juce::StringArray parts;
    for (auto value : values) parts.add (juce::String (value));
    return parts.joinIntoString (",");
}
}

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
    boolean("layerAEnabled","Layer A enabled",true);boolean("layerBEnabled","Layer B enabled");
    number("layerAGain","Layer A level",0,1,1,1,.01f);number("layerBGain","Layer B level",0,1,.5f,1,.01f);
    number("layerAPan","Layer A pan",-1,1,0,1,.01f);number("layerBPan","Layer B pan",-1,1,0,1,.01f);
    number("layerATranspose","Layer A transpose",-24,24,0,1,1);number("layerBTranspose","Layer B transpose",-24,24,-12,1,1);
    number("layerADetune","Layer A detune",-100,100,0,1,1);number("layerBDetune","Layer B detune",-100,100,0,1,1);
    choice("layerBOsc","Layer B waveform",{"Pulse/Square","Triangle","Noise"},1);
    choice("layerBDuty","Layer B duty",{"12.5%","25%","50%"});
    choice("layerBNoise","Layer B noise",{"4-bit random","1-bit long cycle","1-bit short cycle"});
    number("layerBAttack","Layer B attack",0,5,0,.5f,.001f);number("layerBDecay","Layer B decay",0,5,0,.5f,.001f);
    number("layerBSustain","Layer B sustain",0,1,1,1,.01f);number("layerBRelease","Layer B release",0,5,0,.5f,.001f);
    boolean("layerBVolumeEnabled","Layer B volume sequence");boolean("layerBPitchEnabled","Layer B pitch sequence");boolean("layerBDutyEnabled","Layer B duty sequence");
    choice("layerBPitchMode","Layer B pitch units",{"Coarse","Fine"});
    choice("layerASoundMode","Layer A sound mode",{"Raw","Clean"});choice("layerBSoundMode","Layer B sound mode",{"Raw","Clean"});
    boolean("fxEnabled","Effects enabled");number("fxDrive","Saturation drive",0,24,0,1,.1f);
    number("fxBits","Bit depth",4,24,24,1,1);number("fxRate","Rate divider",1,32,1,1,1);
    number("fxMix","Effects mix",0,100,100,1,1);
    const juce::StringArray modSources{"Off","Velocity","Mod wheel","Aftertouch","Timbre CC74","LFO 1","LFO 2","Macro 1","Macro 2","Macro 3","Macro 4","LFO 1 unipolar","LFO 2 unipolar"};
    const juce::StringArray modDestinations{"Off","Pitch","Pulse width","Level","Layer balance","FX drive","FX mix"};
    for(int i=1;i<=4;++i){
        const auto n=juce::String(i);
        choice(("mod"+n+"Source").toRawUTF8(),("Mod "+n+" source").toRawUTF8(),modSources);
        choice(("mod"+n+"Dest").toRawUTF8(),("Mod "+n+" destination").toRawUTF8(),modDestinations);
        number(("mod"+n+"Amount").toRawUTF8(),("Mod "+n+" amount").toRawUTF8(),-100,100,0,1,1);
    }
    const juce::StringArray lfoShapes{"Sine","Triangle","Square","Ramp up","Ramp down","Sample & hold"};
    choice("lfo1Shape","LFO 1 shape",lfoShapes);number("lfo1Rate","LFO 1 rate",.01f,20,1,.5f,.01f);boolean("lfo1Retrigger","LFO 1 retrigger");
    choice("lfo2Shape","LFO 2 shape",lfoShapes);number("lfo2Rate","LFO 2 rate",.01f,20,1,.5f,.01f);boolean("lfo2Retrigger","LFO 2 retrigger");
    number("macro1","Macro 1",0,1,0,1,.01f);number("macro2","Macro 2",0,1,0,1,.01f);
    number("macro3","Macro 3",0,1,0,1,.01f);number("macro4","Macro 4",0,1,0,1,.01f);
    boolean("mpeEnabled","MPE enabled");number("mpeBendRange","MPE bend range",0,96,48,1,1);
    return l;
}
RebitProcessor::RebitProcessor()
    : AudioProcessor(BusesProperties().withOutput("Output",juce::AudioChannelSet::stereo(),true)),
      state(*this,nullptr,"RebitRustParams",layout()), engines{{rebit_engine_new(),rebit_engine_new_seeded(0xcc7530ed)}}, mixer(rebit_mixer_new()), canonical{{rebit_bank_new(),rebit_bank_new()}} {
    parameterValues.reserve((size_t)getParameters().size());
    for(auto* parameter:getParameters())if(auto* identified=dynamic_cast<juce::AudioProcessorParameterWithID*>(parameter))
        parameterValues.emplace(identified->paramID.toRawUTF8(),state.getRawParameterValue(identified->paramID));
}
RebitProcessor::~RebitProcessor() {
    // Hosts stop processing before destruction. No callback owns these handles now.
    for(size_t i=0;i<2;++i){rebit_engine_free(engines[i]);rebit_bank_free(canonical[i]);rebit_bank_free(pending[i]);}rebit_mixer_free(mixer);
}
const char* RebitProcessor::layerParameter(const char* id,int layer) {
    if(layer==0)return id;
    static constexpr std::pair<std::string_view,const char*> ids[]={
        {"osc","layerBOsc"},{"duty","layerBDuty"},{"noiseAlgorithm_raw","layerBNoise"},
        {"attack","layerBAttack"},{"decay","layerBDecay"},{"suslevel","layerBSustain"},{"release","layerBRelease"},
        {"isVolumeSequenceEnabled_raw","layerBVolumeEnabled"},{"isPitchSequenceEnabled_raw","layerBPitchEnabled"},
        {"isDutySequenceEnabled_raw","layerBDutyEnabled"},{"pitchSequenceMode_raw","layerBPitchMode"}};
    for(const auto& entry:ids)if(entry.first==id)return entry.second;
    return id;
}
float RebitProcessor::value(const char* id) const {
    const auto found=parameterValues.find(id);
    return found!=parameterValues.end()&&found->second?found->second->load(std::memory_order_relaxed):0.f;
}
void RebitProcessor::readParameters(bool readTransport) {
    RustParameters p{};
    p.osc=(int)value("osc");p.duty=(int)value("duty");p.noise_algorithm=(int)value("noiseAlgorithm_raw");p.max_poly=(int)value("maxPoly");p.behavior=(int)value("monophonicBehavior_raw");
    p.gain=value("gain");p.attack=value("attack");p.decay=value("decay");p.sustain=value("suslevel");p.release=value("release");
    p.portamento=value("portamentoTime");p.bend_range=value("bendRange");p.vibrato_period=value("vibratoRate");p.vibrato_depth=value("vibratoDepth");p.vibrato_delay=value("vibratoDelay");p.ignores_wheel=value("vibratoIgnoresWheel_raw")>.5f;
    p.sweep_initial=value("sweepInitialPitch");p.sweep_time=value("sweepTime");p.interval=value("arpeggioIntervalSliderValue");p.interval_type=(int)value("arpeggioIntervalType_raw");
    p.volume_enabled=value("isVolumeSequenceEnabled_raw")>.5f;p.pitch_enabled=value("isPitchSequenceEnabled_raw")>.5f;p.duty_enabled=value("isDutySequenceEnabled_raw")>.5f;p.pitch_mode=(int)value("pitchSequenceMode_raw");
    if(value("mpeEnabled")>.5f)p.bend_range=value("mpeBendRange");
    RustModulationParameters modulation{};
    for(int i=0;i<4;++i){
        const auto n=juce::String(i+1);
        auto& route=modulation.routes[i];
        route.source=(int)value(("mod"+n+"Source").toRawUTF8());
        route.destination=(int)value(("mod"+n+"Dest").toRawUTF8());
        route.amount=value(("mod"+n+"Amount").toRawUTF8())*.01f;
        modulation.macros[(size_t)i]=value(("macro"+n).toRawUTF8());
    }
    const char* lfoShapes[]={"lfo1Shape","lfo2Shape"};const char* lfoRates[]={"lfo1Rate","lfo2Rate"};const char* lfoRetriggers[]={"lfo1Retrigger","lfo2Retrigger"};
    for(int i=0;i<2;++i){
        modulation.lfo[i].shape=(int)value(lfoShapes[i]);
        modulation.lfo[i].rate_hz=value(lfoRates[i]);
        modulation.lfo[i].retrigger=value(lfoRetriggers[i])>.5f;
    }
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
    RustParameters b=p;
    b.osc=(int)value("layerBOsc");b.duty=(int)value("layerBDuty");b.noise_algorithm=(int)value("layerBNoise");
    b.attack=value("layerBAttack");b.decay=value("layerBDecay");b.sustain=value("layerBSustain");b.release=value("layerBRelease");
    b.volume_enabled=value("layerBVolumeEnabled")>.5f;b.pitch_enabled=value("layerBPitchEnabled")>.5f;b.duty_enabled=value("layerBDutyEnabled")>.5f;b.pitch_mode=(int)value("layerBPitchMode");
    RustClockParameters clocks{value("hostSync")>.5f?1:0,(int)value("sequenceClock")};
    for(size_t layer=0;layer<2;++layer){
        auto* engine=engines[layer];rebit_engine_arp_parameters(engine,&arp);
        rebit_engine_modulation(engine,&modulation);
        rebit_engine_parameters(engine,layer==0?&p:&b,transport.bpm);
        rebit_engine_clock_parameters(engine,&clocks);rebit_engine_transport(engine,&transport);
    }
    rebit_engine_sound_mode(engines[0],(int)value("layerASoundMode"));
    rebit_engine_sound_mode(engines[1],(int)value("layerBSoundMode"));
    rebit_engine_tuning(engines[0],value("layerATranspose")+value("layerADetune")*.01);
    rebit_engine_tuning(engines[1],value("layerBTranspose")+value("layerBDetune")*.01);
    const RustLayerMix mixA{value("layerAEnabled")>.5f?1:0,value("layerAGain"),value("layerAPan")};
    const RustLayerMix mixB{value("layerBEnabled")>.5f?1:0,value("layerBGain"),value("layerBPan")};
    rebit_mixer_parameters(mixer,0,&mixA);rebit_mixer_parameters(mixer,1,&mixB);
    const RustEffectsParameters fx{value("fxEnabled")>.5f?1:0,value("fxDrive"),value("fxBits"),value("fxRate"),value("fxMix")*.01f};
    rebit_mixer_effects(mixer,&fx);
    if(transport.valid!=0&&transport.playing==0&&clocks.host_sync!=0&&arp.enabled!=0&&arp.rate!=15)rebit_mixer_clear_effects(mixer);
}
void RebitProcessor::prepareToPlay(double sr,int) {
    preparedSampleRate=std::isfinite(sr)?std::clamp(sr,8000.,384000.):44100.;
    playbackIdleAfterMs.store(250,std::memory_order_relaxed);
    for(auto* engine:engines)rebit_engine_prepare(engine,sr);rebit_mixer_prepare(mixer,sr);readParameters(false);keyboard.reset();publishPlayback();clearClip();
}
void RebitProcessor::publishPlayback() {
    RustPlaybackInfo info{};rebit_mixer_playback(mixer,engines[0],engines[1],&info);
    outputPeak.store(info.peak,std::memory_order_relaxed);outputHeldPeak.store(info.held_peak,std::memory_order_relaxed);
    for(size_t layer=0;layer<2;++layer){
        RustPlaybackInfo activity{};rebit_mixer_layer_playback(mixer,layer,engines[layer],&activity);
        voiceActivity[layer].store(activity.active|(activity.held<<8)|(activity.released<<16)|(activity.fading<<24),std::memory_order_relaxed);
    }
    clipRevision.store(info.clips,std::memory_order_relaxed);
    playbackRevision.fetch_add(1,std::memory_order_relaxed);
}
PlaybackSnapshot RebitProcessor::playbackInfo() const {
    PlaybackSnapshot result;
    result.peak=outputPeak.load(std::memory_order_relaxed);result.heldPeak=outputHeldPeak.load(std::memory_order_relaxed);
    for(size_t layer=0;layer<2;++layer){
        const auto activity=voiceActivity[layer].load(std::memory_order_relaxed);auto& part=result.layers[layer];
        part.active=activity&255;part.held=(activity>>8)&255;part.released=(activity>>16)&255;part.fading=activity>>24;
        result.active+=part.active;result.held+=part.held;result.released+=part.released;result.fading+=part.fading;
    }
    result.clipped=clipRevision.load(std::memory_order_relaxed)!=clipAcknowledged.load(std::memory_order_relaxed);
    result.revision=playbackRevision.load(std::memory_order_relaxed);
    result.idleAfterMs=playbackIdleAfterMs.load(std::memory_order_relaxed);return result;
}
bool RebitProcessor::isBusesLayoutSupported(const BusesLayout& l) const {return l.getMainInputChannelSet().isDisabled()&&(l.getMainOutputChannelSet()==juce::AudioChannelSet::mono()||l.getMainOutputChannelSet()==juce::AudioChannelSet::stereo());}
void RebitProcessor::midi(const juce::MidiMessage& m) {
    const auto* data=m.getRawData();const int n=m.getRawDataSize();
    if(n>2&&(data[0]&0xf0)==0xb0&&data[1]==120)rebit_mixer_clear_effects(mixer);
    if(n>0)for(auto* engine:engines)rebit_engine_midi(engine,data[0],n>1?data[1]:0,n>2?data[2]:0);
}
void RebitProcessor::processBlock(juce::AudioBuffer<float>& buffer,juce::MidiBuffer& messages) {
    juce::ScopedNoDenormals noDenormals;
    playbackIdleAfterMs.store((uint32_t)std::max(250.,3000.*buffer.getNumSamples()/preparedSampleRate),std::memory_order_relaxed);
    if(resetRequested.exchange(false)){for(auto* engine:engines)rebit_engine_reset(engine);rebit_mixer_reset(mixer);}
    readParameters();
    {
        const juce::SpinLock::ScopedTryLockType lock(sequenceLock);
        if(lock.isLocked())for(size_t layer=0;layer<2;++layer)if(sequenceDirty[layer]){
            pending[layer]=rebit_engine_swap_bank(engines[layer],pending[layer]);sequenceDirty[layer]=false;
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
        rebit_mixer_render(mixer,engines[0],engines[1],first,buffer.getNumChannels()>1?buffer.getWritePointer(1)+at:nullptr,(size_t)n);
        at=end;
    };
    for(const auto metadata:messages){render(metadata.samplePosition);midi(metadata.getMessage());}
    render(buffer.getNumSamples());messages.clear();
    publishPlayback();
    clockState.store(rebit_engine_clock_status(engines[0]),std::memory_order_relaxed);
    for(size_t layer=0;layer<2;++layer){
        int32_t positions[3];rebit_engine_positions(engines[layer],positions);
        if(voiceActivity[layer].load(std::memory_order_relaxed)==0)std::fill(std::begin(positions),std::end(positions),-1);
        for(size_t lane=0;lane<3;++lane)playheads[layer*3+lane].store(positions[lane],std::memory_order_relaxed);
    }
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
SequenceSnapshot RebitProcessor::sequenceData(int lane,int layer) const {
    SequenceSnapshot result;if(lane<0||lane>=3||layer<0||layer>=2)return result;
    const juce::ScopedLock lock(textMutex);RustSequenceInfo info{};
    if(!rebit_bank_read(canonical[(size_t)layer],(size_t)lane,nullptr,0,&info))return result;
    result.values.resize(info.count);rebit_bank_read(canonical[(size_t)layer],(size_t)lane,result.values.data(),result.values.size(),&info);
    result.loopStart=info.loop_start;result.releaseStart=info.release_start;result.revision=sequenceVersion[(size_t)layer].load();return result;
}
juce::String RebitProcessor::sequenceText(int lane,int layer) const {
    const juce::ScopedLock lock(textMutex);return lane>=0&&lane<3&&layer>=0&&layer<2?texts[(size_t)layer][(size_t)lane]:juce::String{};
}
juce::String RebitProcessor::setSequenceText(int lane,const juce::String& text,int layer) {
    if(lane<0||lane>=3||layer<0||layer>=2)return "Unknown sequence";
    const auto index=(size_t)layer;const juce::ScopedLock textLock(textMutex);
    auto* next=rebit_bank_clone(canonical[index]);uint8_t error[256]{};const auto utf8=text.toUTF8();
    if(!rebit_bank_set(next,(size_t)lane,reinterpret_cast<const uint8_t*>(utf8.getAddress()),text.getNumBytesAsUTF8(),error,sizeof(error))){
        rebit_bank_free(next);return juce::String::fromUTF8(reinterpret_cast<const char*>(error));
    }
    rebit_bank_free(canonical[index]);canonical[index]=next;auto* publication=rebit_bank_clone(canonical[index]);
    {const juce::SpinLock::ScopedLockType lock(sequenceLock);std::swap(pending[index],publication);sequenceDirty[index]=true;}
    rebit_bank_free(publication);texts[index][(size_t)lane]=text;++sequenceVersion[index];return {};
}
void RebitProcessor::setParameterValue (const char* id, float value) {
    if (auto* parameter = state.getParameter (id))
        parameter->setValueNotifyingHost (parameter->convertTo0to1 (value));
}
void RebitProcessor::resetToDefaults() {
    for (auto* parameter : getParameters())
        if (auto* ranged = dynamic_cast<juce::RangedAudioParameter*> (parameter))
            ranged->setValueNotifyingHost (ranged->getDefaultValue());
    for (int layer = 0; layer < 2; ++layer)
        for (int lane = 0; lane < 3; ++lane)
            setSequenceText (lane, juce::String(), layer);
}
void RebitProcessor::pushUndo() {
    juce::MemoryBlock snapshot;
    getStateInformation (snapshot);
    if (undoStack.size() >= 8) undoStack.erase (undoStack.begin());
    undoStack.push_back (std::move (snapshot));
}
int RebitProcessor::presetCount() const { return (int) factoryPresets().size(); }
juce::String RebitProcessor::presetLabel (int index) const {
    const auto& presets = factoryPresets();
    if (index < 0 || index >= (int) presets.size()) return {};
    const auto& preset = presets[(size_t) index];
    if (preset.category == preset.name) return juce::String (preset.name);
    return juce::String (preset.category) + " \xe2\x80\x94 " + juce::String (preset.name);
}
void RebitProcessor::loadPreset (int index) {
    const auto& presets = factoryPresets();
    if (index < 0 || index >= (int) presets.size()) return;
    const auto& preset = presets[(size_t) index];
    pushUndo();
    resetToDefaults();
    for (const auto& parameter : preset.parameters) setParameterValue (parameter.id, parameter.value);
    const char* laneIds[] = { "isVolumeSequenceEnabled_raw", "isPitchSequenceEnabled_raw", "isDutySequenceEnabled_raw" };
    for (int layer = 0; layer < 2; ++layer) {
        const auto& sequences = layer == 0 ? preset.sequenceA : preset.sequenceB;
        const auto& lanes = layer == 0 ? preset.lanesA : preset.lanesB;
        for (int lane = 0; lane < 3; ++lane) {
            setSequenceText (lane, juce::String (sequences[(size_t) lane]), layer);
            setParameterValue (layerParameter (laneIds[lane], layer), lanes[(size_t) lane] ? 1.f : 0.f);
        }
    }
    resetRequested.store (true);
}
void RebitProcessor::undo() {
    if (undoStack.empty()) return;
    auto snapshot = std::move (undoStack.back());
    undoStack.pop_back();
    setStateInformation (snapshot.getData(), (int) snapshot.getSize());
}
void RebitProcessor::randomize (uint32_t seed, int lockedSections) {
    lockedSections &= RandomAll;
    if (lockedSections == RandomAll) return;
    pushUndo();
    DeterministicRandom random (seed);
    auto set = [this] (const char* id, float value) { setParameterValue (id, value); };
    auto chance = [&random] (float probability) { return random.chance (probability); };
    auto range = [&random] (int lo, int hi) { return random.range (lo, hi); };
    auto value = [&random] (float lo, float hi) { return random.range (lo, hi); };
    const char* laneIds[] = { "isVolumeSequenceEnabled_raw", "isPitchSequenceEnabled_raw", "isDutySequenceEnabled_raw" };
    if ((lockedSections & RandomOsc) == 0) {
        set ("osc", (float) range (0, 2));
        set ("duty", (float) range (0, 2));
        set ("noiseAlgorithm_raw", (float) range (0, 2));
        set ("layerBOsc", (float) range (0, 2));
        set ("layerBDuty", (float) range (0, 2));
        set ("layerBNoise", (float) range (0, 2));
        set ("layerASoundMode", chance (0.35f) ? 1.f : 0.f);
        set ("layerBSoundMode", chance (0.35f) ? 1.f : 0.f);
    }
    if ((lockedSections & RandomEnv) == 0) {
        auto envelope = [&] (const char* attack, const char* decay, const char* sustain, const char* release) {
            set (attack, chance (0.72f) ? value (0.f, 0.08f) : value (0.1f, 1.2f));
            set (decay, value (0.02f, 1.2f));
            set (sustain, value (0.15f, 1.f));
            set (release, value (0.02f, 1.4f));
        };
        envelope ("attack", "decay", "suslevel", "release");
        envelope ("layerBAttack", "layerBDecay", "layerBSustain", "layerBRelease");
    }
    if ((lockedSections & RandomSeq) == 0) {
        set ("sequenceClock", (float) range (0, 7));
        set ("pitchSequenceMode_raw", chance (0.25f) ? 1.f : 0.f);
        const int scale[] = { 0, 2, 3, 5, 7, 10, 12 };
        for (int layer = 0; layer < 2; ++layer) {
            for (int lane = 0; lane < 3; ++lane) {
                juce::Array<int> main, release;
                if (lane == 0) {
                    for (int i = 0; i < 8; ++i) main.add (chance (0.18f) ? range (0, 12) : 15);
                    release.add (0); release.add (0);
                } else if (lane == 1) {
                    const int root = range (-12, 12);
                    for (int i = 0; i < 8; ++i) {
                        const int note = root + scale[range (0, 6)] + (chance (0.25f) ? 12 : 0);
                        main.add (juce::jlimit (-64, 63, note));
                    }
                    release.add (0);
                } else {
                    for (int i = 0; i < 8; ++i) main.add (range (0, 2));
                    release.add (0);
                }
                const auto text = "[" + join (main) + "]|" + join (release);
                setSequenceText (lane, text, layer);
                const float laneChance = lane == 0 ? 0.65f : lane == 1 ? 0.55f : 0.35f;
                setParameterValue (layerParameter (laneIds[lane], layer), chance (laneChance) ? 1.f : 0.f);
            }
        }
    }
    if ((lockedSections & RandomMod) == 0) {
        const int activeRoutes = range (1, 3);
        for (int i = 0; i < 4; ++i) {
            const auto n = juce::String (i + 1);
            if (i < activeRoutes) {
                set (("mod" + n + "Source").toRawUTF8(), (float) range (1, 12));
                set (("mod" + n + "Dest").toRawUTF8(), (float) range (1, 6));
                set (("mod" + n + "Amount").toRawUTF8(), value (-75.f, 75.f));
            } else {
                set (("mod" + n + "Source").toRawUTF8(), 0.f);
                set (("mod" + n + "Dest").toRawUTF8(), 0.f);
                set (("mod" + n + "Amount").toRawUTF8(), 0.f);
            }
        }
        for (int i = 0; i < 2; ++i) {
            const auto n = juce::String (i + 1);
            set (("lfo" + n + "Shape").toRawUTF8(), (float) range (0, 5));
            set (("lfo" + n + "Rate").toRawUTF8(), chance (0.6f) ? value (0.1f, 2.f) : value (1.f, 8.f));
            set (("lfo" + n + "Retrigger").toRawUTF8(), chance (0.4f) ? 1.f : 0.f);
        }
        for (int i = 0; i < 4; ++i)
            set (("macro" + juce::String (i + 1)).toRawUTF8(), chance (0.25f) ? 0.f : value (0.2f, 1.f));
    }
    if ((lockedSections & RandomFx) == 0) {
        const bool enabled = chance (0.55f);
        set ("fxEnabled", enabled ? 1.f : 0.f);
        set ("fxDrive", value (0.f, 18.f));
        set ("fxBits", (float) range (6, 24));
        set ("fxRate", (float) range (1, 12));
        set ("fxMix", value (25.f, 100.f));
    }
    if ((lockedSections & RandomMix) == 0) {
        const int polys[] = { 1, 1, 4, 8, 16 };
        set ("maxPoly", (float) polys[range (0, 4)]);
        set ("gain", value (0.25f, 0.6f));
        set ("layerAEnabled", 1.f);
        set ("layerBEnabled", chance (0.55f) ? 1.f : 0.f);
        set ("layerAGain", value (0.4f, 1.f));
        set ("layerBGain", value (0.3f, 0.9f));
        set ("layerAPan", value (-0.5f, 0.5f));
        set ("layerBPan", value (-0.5f, 0.5f));
        set ("layerATranspose", (float) range (-12, 12));
        set ("layerBTranspose", (float) range (-24, 24));
        set ("layerADetune", value (-12.f, 12.f));
        set ("layerBDetune", value (-15.f, 15.f));
    }
}

void RebitProcessor::getStateInformation(juce::MemoryBlock& out) {
    const juce::ScopedLock lock(textMutex);auto xml=std::make_unique<juce::XmlElement>("RebitRust");xml->setAttribute("version",4);
    xml->addChildElement(state.copyState().createXml().release());const char* names[]={"volume","pitch","duty"};
    for(size_t layer=0;layer<2;++layer){
        auto* node=layer==0?xml.get():xml->createNewChildElement("layerB");
        for(size_t lane=0;lane<3;++lane)node->createNewChildElement(names[lane])->addTextElement(texts[layer][lane]);
    }
    copyXmlToBinary(*xml,out);
}
void RebitProcessor::setStateInformation(const void* data,int size) {
    auto xml=getXmlFromBinary(data,size);if(!xml||!xml->hasTagName("RebitRust"))return;
    const int version=xml->getIntAttribute("version");if(version<1||version>4)return;
    auto* params=xml->getChildByName("RebitRustParams");if(!params)return;
    const juce::ScopedLock textLock(textMutex);
    std::array<RustSequenceBank*,2> next{{rebit_bank_new(),rebit_bank_new()}};
    std::array<std::array<juce::String,3>,2> restored;
    const char* names[]={"volume","pitch","duty"};
    for(size_t layer=0;layer<2;++layer)for(size_t lane=0;lane<3;++lane){
        auto* node=layer==0?xml.get():version>=2?xml->getChildByName("layerB"):nullptr;
        auto* child=node?node->getChildByName(names[lane]):nullptr;restored[layer][lane]=child?child->getAllSubText():juce::String{};
        const auto utf8=restored[layer][lane].toUTF8();uint8_t error[256]{};
        if(!rebit_bank_set(next[layer],lane,reinterpret_cast<const uint8_t*>(utf8.getAddress()),restored[layer][lane].getNumBytesAsUTF8(),error,sizeof(error))){
            for(auto* bank:next)rebit_bank_free(bank);return;
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
    // Version-1 patches are single-layer. Never inherit current layer settings.
    for(auto* parameter:getParameters())if(auto* p=dynamic_cast<juce::RangedAudioParameter*>(parameter))
        if(p->paramID.startsWith("layer")){
            auto child=parameterTree.getChildWithProperty("id",p->paramID);
            if(version==1&&child.isValid()){parameterTree.removeChild(child,nullptr);child={};}
            if(!child.isValid()){
                juce::ValueTree defaults("PARAM");defaults.setProperty("id",p->paramID,nullptr);
                defaults.setProperty("value",p->convertFrom0to1(p->getDefaultValue()),nullptr);parameterTree.addChild(defaults,-1,nullptr);
            }
        }
    // Earlier formats have neither Clean mode nor effects. Do not inherit the
    // currently active colour settings when recalling a legacy sound.
    for(auto* parameter:getParameters())if(auto* p=dynamic_cast<juce::RangedAudioParameter*>(parameter))
        if(p->paramID.startsWith("fx")||p->paramID.endsWith("SoundMode")){
            auto child=parameterTree.getChildWithProperty("id",p->paramID);
            if(version<3&&child.isValid()){parameterTree.removeChild(child,nullptr);child={};}
            if(!child.isValid()){
                juce::ValueTree defaults("PARAM");defaults.setProperty("id",p->paramID,nullptr);
                defaults.setProperty("value",p->convertFrom0to1(p->getDefaultValue()),nullptr);parameterTree.addChild(defaults,-1,nullptr);
            }
        }
    // Version-4 adds the modulation matrix, LFOs, macros and MPE. Legacy
    // patches must not inherit whatever happens to be loaded in the editor.
    for(auto* parameter:getParameters())if(auto* p=dynamic_cast<juce::RangedAudioParameter*>(parameter))
        if(p->paramID.startsWith("mod")||p->paramID.startsWith("lfo")||p->paramID.startsWith("macro")||p->paramID.startsWith("mpe")){
            auto child=parameterTree.getChildWithProperty("id",p->paramID);
            if(version<4&&child.isValid()){parameterTree.removeChild(child,nullptr);child={};}
            if(!child.isValid()){
                juce::ValueTree defaults("PARAM");defaults.setProperty("id",p->paramID,nullptr);
                defaults.setProperty("value",p->convertFrom0to1(p->getDefaultValue()),nullptr);parameterTree.addChild(defaults,-1,nullptr);
            }
        }
    state.replaceState(parameterTree);
    std::array<RustSequenceBank*,2> publication{};
    for(size_t layer=0;layer<2;++layer){rebit_bank_free(canonical[layer]);canonical[layer]=next[layer];publication[layer]=rebit_bank_clone(canonical[layer]);}
    texts=restored;
    {const juce::SpinLock::ScopedLockType lock(sequenceLock);for(size_t layer=0;layer<2;++layer){std::swap(pending[layer],publication[layer]);sequenceDirty[layer]=true;}}
    for(auto* bank:publication)rebit_bank_free(bank);
    for(auto& revision:sequenceVersion)++revision;resetRequested=true;
}
juce::AudioProcessorEditor* RebitProcessor::createEditor(){return new RebitEditor(*this);}
juce::AudioProcessor* JUCE_CALLTYPE createPluginFilter(){return new RebitProcessor();}
