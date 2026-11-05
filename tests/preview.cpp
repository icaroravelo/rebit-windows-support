#include "PluginProcessor.h"
#include <iostream>
#include "TestPlayHead.h"
int main(int argc,char** argv) {
    juce::ScopedJuceInitialiser_GUI init;
    TestPlayHead host;
    auto processor=std::make_unique<RebitProcessor>();
    // Optional display state for visual QA; plugin defaults and saved patches
    // are unaffected. Every displayed control still uses its real attachment.
    const auto mode=argc>2?juce::String(argv[2]):juce::String{};
    const bool colour=mode=="--colour";
    const bool modPage=mode=="--mod";
    const bool lfoPage=mode=="--lfo";
    const bool macroPage=mode=="--macros";
    const bool randomPage=mode=="--random";
    const bool userPage=mode=="--user";
    const bool dual=colour||modPage||lfoPage||macroPage||mode=="--showcase"||mode=="--layer-b";
    if(dual) {
        auto set=[&](const char* id,float value){if(auto* p=processor->state.getParameter(id))p->setValueNotifyingHost(p->convertTo0to1(value));};
        host.set(0.,120.,true);processor->setPlayHead(&host);set("hostSync",1);set("sequenceClock",4);
        set("arpEnabled",1);set("arpPattern",2);set("arpOctaves",1);set("arpSwing",18);set("arpGate",75);
        set("maxPoly",8);set("attack",.015f);set("decay",.25f);set("suslevel",.65f);set("release",.35f);
        set("duty",1);set("portamentoTime",.08f);set("vibratoDepth",.35f);set("sweepInitialPitch",7);set("sweepTime",.16f);
        set("layerBEnabled",1);set("layerAGain",.8f);set("layerBGain",.45f);set("layerAPan",-.25f);set("layerBPan",.35f);
        set("layerBDetune",-7);set("layerBAttack",.025f);set("layerBDecay",.18f);set("layerBSustain",.75f);set("layerBRelease",.4f);
        processor->setSequenceText(0,"15[12,9,12]|6,3,0",1);processor->setSequenceText(1,"[0,-12,0,7]|0",1);
        set("layerBVolumeEnabled",1);set("layerBPitchEnabled",1);
        if(colour){set("layerASoundMode",1);set("layerBSoundMode",1);set("fxEnabled",1);set("fxDrive",9);set("fxBits",8);set("fxRate",4);set("fxMix",65);}
        set("mod1Source",2);set("mod1Dest",1);set("mod1Amount",35);
        set("mod2Source",5);set("mod2Dest",2);set("mod2Amount",45);
        set("mod3Source",7);set("mod3Dest",4);set("mod3Amount",-30);
        set("lfo1Shape",1);set("lfo1Rate",3.2f);set("lfo1Retrigger",1);
        set("lfo2Shape",5);set("lfo2Rate",.7f);
        set("macro1",.6f);set("macro2",.3f);set("macro3",.8f);
        processor->setSequenceText(0,"15,12,9[7,5]|3,1,0");
        processor->setSequenceText(1,"0,4,7[12,7,4]|0");
        processor->setSequenceText(2,"0,1[2,1]|0");
        set("isVolumeSequenceEnabled_raw",1);set("isPitchSequenceEnabled_raw",1);set("isDutySequenceEnabled_raw",1);
        processor->prepareToPlay(48000,128);
        juce::AudioBuffer<float> audio(2,128);juce::MidiBuffer midi;
        midi.addEvent(juce::MidiMessage::noteOn(1,60,(juce::uint8)100),0);
        midi.addEvent(juce::MidiMessage::noteOn(1,67,(juce::uint8)100),0);
        for(int block=0;block<32;++block){host.set((double)block*128./24000.,120.,true);processor->processBlock(audio,midi);}
    }
    if(argc>2 && juce::String(argv[2])=="--clip") {
        auto* release=processor->state.getParameter("release");release->setValueNotifyingHost(release->convertTo0to1(.2f));
        processor->prepareToPlay(8000,64);juce::AudioBuffer<float> audio(2,64);juce::MidiBuffer midi;
        for(int note:{0,1,2})midi.addEvent(juce::MidiMessage::noteOn(1,note,(juce::uint8)127),0);
        midi.addEvent(juce::MidiMessage::noteOff(1,0),32);processor->processBlock(audio,midi);
    }
    juce::File userPreviewDirectory;
    if(userPage){
        userPreviewDirectory=juce::File::getSpecialLocation(juce::File::tempDirectory)
            .getChildFile("rebit-rust-preview-user-presets");
        userPreviewDirectory.deleteRecursively();
        processor->setUserPresetDirectory(userPreviewDirectory);
        processor->saveUserPreset("Neon Bass");
    }
    auto editor=std::unique_ptr<juce::AudioProcessorEditor>(processor->createEditor());
    if(argc>2&&juce::String(argv[2])=="--layer-b")for(auto* child:editor->getChildren())
        if(child->getName()=="Edit layer B")if(auto* button=dynamic_cast<juce::TextButton*>(child))button->onClick();
    auto clickPage=[&](const juce::String& name){for(auto* child:editor->getChildren())if(child->getName()==name)if(auto* button=dynamic_cast<juce::TextButton*>(child))button->onClick();};
    if(modPage||lfoPage||macroPage)clickPage("Modulation page");
    if(lfoPage)clickPage("LFO page");
    if(macroPage)clickPage("Macros page");
    if(userPage){
        for(auto* child:editor->getChildren())
            if(child->getName()=="Preset browser")
                if(auto* box=dynamic_cast<juce::ComboBox*>(child))
                    box->setSelectedItemIndex(box->getNumItems()-1,juce::sendNotificationSync);
    }
    if(randomPage){
        for(auto* child:editor->getChildren()){
            if(child->getName()=="Random seed")if(auto* field=dynamic_cast<juce::TextEditor*>(child))field->setText("20261105",false);
            if(child->getName()=="Lock ENV")if(auto* lock=dynamic_cast<juce::ToggleButton*>(child))lock->setToggleState(true,juce::dontSendNotification);
            if(child->getName()=="Randomize")if(auto* button=dynamic_cast<juce::TextButton*>(child))button->onClick();
        }
    }
    editor->setVisible(true);
    auto image=editor->createComponentSnapshot(editor->getLocalBounds(),true,1.5f);
    juce::File output(argc>1?argv[1]:"/tmp/rebit-preview.png");
    juce::FileOutputStream stream(output);
    stream.setPosition(0);stream.truncate();
    juce::PNGImageFormat png;
    if(!stream.openedOk()||!png.writeImageToStream(image,stream))return 1;
    std::cout<<output.getFullPathName()<<std::endl;
    if(userPreviewDirectory != juce::File()) userPreviewDirectory.deleteRecursively();
}
