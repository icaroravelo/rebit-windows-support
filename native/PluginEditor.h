#pragma once
#include <array>
#include <memory>
#include <vector>
#include <juce_audio_utils/juce_audio_utils.h>
#include "PluginProcessor.h"

class SequenceCanvas;

class RebitEditor final : public juce::AudioProcessorEditor, private juce::Timer
{
public:
    explicit RebitEditor (RebitProcessor&);
    ~RebitEditor() override;
    void paint (juce::Graphics&) override;
    void resized() override;

private:
    struct Knob;
    struct Choice;
    struct Keyboard;
    struct PlaybackDisplay;
    class RebitLookAndFeel;
    enum KnobIndex { gain, poly, attack, decay, sustain, release, portamento, bend,
                     arpSpeed, vibratoRate, vibratoDepth, vibratoDelay, sweepPitch, sweepTime, arpGate, arpSwing, arpSeconds };
    enum ChoiceIndex { oscillator, pulseDuty, noiseAlgorithm, monoBehavior, arpInterval, pitchMode, arpPattern, arpRate, arpOctaves, arpArticulation, sequenceClock };
    RebitProcessor& instrument;
    std::unique_ptr<RebitLookAndFeel> theme;
    std::vector<std::unique_ptr<Knob>> knobs;
    std::vector<std::unique_ptr<Choice>> choices;
    juce::ToggleButton ignoreWheel { "Independent of mod wheel" };
    std::array<juce::ToggleButton, 3> sequenceEnabled;
    std::array<juce::TextEditor, 3> sequenceEditors;
    std::array<juce::String, 3> lastCommittedSequences;
    juce::TextButton applySequences { "Apply sequences" };
    juce::Label sequenceStatus;
    std::unique_ptr<SequenceCanvas> sequenceCanvas;
    std::unique_ptr<Keyboard> keyboard;
    std::unique_ptr<PlaybackDisplay> playbackDisplay;
    juce::TooltipWindow tooltips { this, 650 };
    bool midiActive = false;
    bool monoMode = false;
    bool arpPanel = true;
    int selectedLayer=0;
    bool switchingLayer=false;
    juce::TextButton editA {"EDIT A"}, editB {"EDIT B"};
    juce::ToggleButton layerAOn {"A on"},layerBOn {"B on"};
    std::array<std::unique_ptr<Knob>,4> layerKnobs;
    std::array<std::unique_ptr<juce::AudioProcessorValueTreeState::ButtonAttachment>,3> laneAttachments;
    void selectLayer(int);
    std::unique_ptr<Choice> soundMode;
    std::array<std::unique_ptr<Knob>,4> effectKnobs;
    juce::ToggleButton effectsOn {"FX on"};
    juce::TextButton arpTab { "ARP" }, monoTab { "MONO" }, arpStop { "STOP" };
    juce::ToggleButton arpEnabled { "Arp on" }, arpLatch { "Latch" }, hostSync { "Host sync" };
    void updatePerformance();
    std::vector<std::unique_ptr<juce::AudioProcessorValueTreeState::ButtonAttachment>> toggles;
    void timerCallback() override;
    void applySequence (int);
    void applyAllSequences();
    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (RebitEditor)
};

