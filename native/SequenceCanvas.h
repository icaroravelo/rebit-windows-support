#pragma once
#include "PluginProcessor.h"

// Message-thread editor. It never reads the audio-owned Rust engine.
class SequenceCanvas final : public juce::Component, private juce::Timer {
public:
    explicit SequenceCanvas(RebitProcessor&);
    ~SequenceCanvas() override;
    void paint(juce::Graphics&) override;
    void resized() override;
    void mouseDown(const juce::MouseEvent&) override;
    void mouseDrag(const juce::MouseEvent&) override;
    void mouseUp(const juce::MouseEvent&) override;
    bool keyPressed(const juce::KeyPress&) override;
    int lane() const { return selectedLane; }
    void selectLane(int);
    void selectLayer(int);
    std::function<void(int)> onLaneChanged;
    std::function<void(int)> onCommitted;
private:
    RebitProcessor& processor;
    SequenceSnapshot data;
    uint64_t observedRevision=~uint64_t{};
    int selectedLayer=0, selectedLane=0, page=0, selectedStep=0, lastStep=-1, pitchRange=12;
    int playhead=-1, dragMarker=0;
    bool dirty=false, drawing=false;
    std::array<juce::TextButton,3> tabs;
    juce::TextButton loop{"LOOP"}, release{"RELEASE"}, range{"+/-12"}, previous{"<"}, next{">"};
    juce::Label stepsLabel, pageLabel, valueLabel;
    juce::TextEditor length;
    juce::Rectangle<float> grid() const;
    juce::Colour accent() const;
    int minValue() const;
    int maxValue() const;
    int stepAt(float) const;
    float stepX(int) const;
    void seed();
    void resizePattern();
    void controls();
    void drawAt(juce::Point<float>);
    void commit();
    void refresh();
    void timerCallback() override;
    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR(SequenceCanvas)
};
