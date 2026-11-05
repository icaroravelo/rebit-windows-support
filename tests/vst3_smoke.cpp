#include <juce_audio_processors/juce_audio_processors.h>
#include <juce_gui_basics/juce_gui_basics.h>
#include <iostream>
#include "TestPlayHead.h"
int main(int argc,char** argv) {
    juce::ScopedJuceInitialiser_GUI init;
    if(argc<2)return 2;
    juce::AudioPluginFormatManager manager;
    auto format=std::make_unique<juce::VST3PluginFormat>();
    juce::OwnedArray<juce::PluginDescription> descriptions;
    format->findAllTypesForFile(descriptions,argv[1]);
    manager.addFormat(std::move(format));
    if(descriptions.isEmpty()){std::cerr<<"No VST3 instruments found\n";return 1;}
    juce::String error;
    auto plugin=manager.createPluginInstance(*descriptions[0],44100,64,error);
    if(!plugin){std::cerr<<error<<std::endl;return 1;}
    plugin->setPlayConfigDetails(0,2,44100,64);
    plugin->prepareToPlay(44100,64);
    juce::AudioBuffer<float> buffer(2,64);buffer.clear();
    juce::MidiBuffer midi;midi.addEvent(juce::MidiMessage::noteOn(1,69,(juce::uint8)127),0);
    plugin->processBlock(buffer,midi);
    if(std::abs(buffer.getSample(0,0)+.5f)>1e-7)return 1;
    for(int i=0;i<64;++i)if(buffer.getSample(0,i)!=buffer.getSample(1,i))return 1;
    juce::MemoryBlock state;plugin->getStateInformation(state);
    if(state.getSize()==0)return 1;
    plugin->setStateInformation(state.getData(),(int)state.getSize());
    buffer.clear();midi.clear();midi.addEvent(juce::MidiMessage::noteOn(1,69,(juce::uint8)127),0);
    plugin->processBlock(buffer,midi);
    if(std::abs(buffer.getSample(0,0)+.5f)>1e-7)return 1;
    plugin->releaseResources();
    TestPlayHead host;host.set(.1,120.,true);plugin->setPlayHead(&host);
    auto set=[&](const char* name,float value){for(auto* parameter:plugin->getParameters())if(parameter->getName(64)==name){parameter->setValueNotifyingHost(value);return true;}return false;};
    if(!set("Arpeggiator",1.f)||!set("Host sync",1.f))return 1;
    plugin->setPlayConfigDetails(0,2,8000,1200);plugin->prepareToPlay(8000,1200);
    buffer.setSize(2,1200);midi.clear();midi.addEvent(juce::MidiMessage::noteOn(1,60,(juce::uint8)127),17);
    plugin->processBlock(buffer,midi);
    for(int i=0;i<600;++i)if(buffer.getSample(0,i)!=0.f){std::cerr<<"VST3 did not forward PPQ grid\n";return 1;}
    if(std::abs(buffer.getSample(0,600))<.49f){std::cerr<<"VST3 missed host grid\n";return 1;}
    host.set(.4,120.,false);midi.clear();plugin->processBlock(buffer,midi);
    if(buffer.getMagnitude(0,1200)>1e-7f){std::cerr<<"VST3 host stop failed\n";return 1;}
    plugin->releaseResources();plugin->setPlayHead(nullptr);
    if(!set("Arpeggiator",0)||!set("Layer B enabled",1)||!set("Layer B level",1)||!set("Layer A pan",0)||!set("Layer B pan",1)||!set("Layer B transpose",.5f))return 1;
    plugin->prepareToPlay(8000,128);buffer.setSize(2,128);midi.clear();midi.addEvent(juce::MidiMessage::noteOn(1,0,(juce::uint8)127),0);
    plugin->processBlock(buffer,midi);
    if(std::abs(buffer.getSample(0,0)+.5f)>1e-6f||std::abs(buffer.getSample(1,0)+8.f/15.f)>1e-6f){std::cerr<<"VST3 stereo layer routing failed\n";return 1;}
    plugin->releaseResources();
    if(!set("Layer B enabled",0)||!set("Layer A pan",.5f)||!set("Layer A sound mode",1)||!set("Effects enabled",1)||!set("Saturation drive",.5f)||!set("Bit depth",.2f)||!set("Rate divider",7.f/31.f))return 1;
    plugin->prepareToPlay(8000,128);midi.clear();midi.addEvent(juce::MidiMessage::noteOn(1,0,(juce::uint8)127),0);plugin->processBlock(buffer,midi);
    for(int i=0;i<8;++i)if(std::abs(buffer.getSample(0,i))>1e-7f){std::cerr<<"VST3 Clean edge/sample hold failed\n";return 1;}
    if(std::abs(buffer.getSample(0,8))<.5f){std::cerr<<"VST3 effects produced no saturation\n";return 1;}
    for(int i=8;i<16;++i)if(std::abs(buffer.getSample(0,i)-buffer.getSample(0,8))>1e-7f)return 1;
    plugin->setPlayHead(nullptr);plugin->releaseResources();
    std::cout<<"VST3 discovery, instantiation, MIDI, stereo render, state recall, host PPQ/start-stop: passed\n";
}
