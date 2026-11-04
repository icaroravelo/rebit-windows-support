#pragma once
#include <juce_audio_utils/juce_audio_utils.h>
#include "RustEngine.h"
#include <vector>

struct SequenceSnapshot {
    std::vector<int16_t> values;
    int loopStart=-1, releaseStart=-1;
    uint64_t revision=0;
    static juce::String toText(const SequenceSnapshot&);
};
class RebitProcessor final : public juce::AudioProcessor {
public:
    RebitProcessor();
    ~RebitProcessor() override;
    void prepareToPlay(double,int) override;
    void releaseResources() override {}
    bool isBusesLayoutSupported(const BusesLayout&) const override;
    void processBlock(juce::AudioBuffer<float>&,juce::MidiBuffer&) override;
    juce::AudioProcessorEditor* createEditor() override;
    bool hasEditor() const override { return true; }
    const juce::String getName() const override { return "Rebit Rust"; }
    bool acceptsMidi() const override { return true; }
    bool producesMidi() const override { return false; }
    bool isMidiEffect() const override { return false; }
    double getTailLengthSeconds() const override { return 5.0; }
    int getNumPrograms() override { return 1; }
    int getCurrentProgram() override { return 0; }
    void setCurrentProgram(int) override {}
    const juce::String getProgramName(int) override { return "Init"; }
    void changeProgramName(int,const juce::String&) override {}
    void getStateInformation(juce::MemoryBlock&) override;
    void setStateInformation(const void*,int) override;
    juce::String sequenceText(int) const;
    SequenceSnapshot sequenceData(int) const;
    uint64_t sequenceRevision() const { return sequenceVersion.load(); }
    int sequencePosition(int lane) const { return lane>=0 && lane<3 ? playheads[(size_t)lane].load(std::memory_order_relaxed) : -1; }
    int clockStatus() const { return clockState.load(std::memory_order_relaxed); }
    juce::String setSequenceText(int,const juce::String&);
    void stopArpeggiator() { keyboard.reset();resetRequested.store(true); } // Message thread.
    juce::AudioProcessorValueTreeState state;
    juce::MidiKeyboardState keyboard;
private:
    static juce::AudioProcessorValueTreeState::ParameterLayout layout();
    float value(const char*) const;
    void readParameters(bool readTransport=true);
    void midi(const juce::MidiMessage&);
    RustEngine* engine = nullptr;
    RustSequenceBank* canonical = nullptr;
    mutable juce::CriticalSection textMutex;
    std::array<juce::String,3> texts{};
    juce::SpinLock sequenceLock;
    RustSequenceBank* pending = nullptr;
    bool sequenceDirty = false;
    std::atomic<uint64_t> sequenceVersion { 0 };
    std::array<std::atomic<int>,3> playheads {{ -1, -1, -1 }};
    static_assert(std::atomic<int>::is_always_lock_free);
    std::atomic<int> clockState { 3 };
    std::atomic<bool> resetRequested{false};
    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR(RebitProcessor)
};
