#pragma once
#include <juce_audio_utils/juce_audio_utils.h>
#include "RustEngine.h"
#include <vector>
#include <string_view>
#include <unordered_map>

struct SequenceSnapshot {
    std::vector<int16_t> values;
    int loopStart=-1, releaseStart=-1;
    uint64_t revision=0;
    static juce::String toText(const SequenceSnapshot&);
};
struct LayerActivity {uint32_t active=0,held=0,released=0,fading=0;};
struct PlaybackSnapshot {
    std::array<LayerActivity,2> layers{};
    float peak=0.f,heldPeak=0.f;
    uint32_t active=0,held=0,released=0,fading=0,revision=0,idleAfterMs=250;
    bool clipped=false;
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
    juce::String sequenceText(int,int layer=0) const;
    SequenceSnapshot sequenceData(int,int layer=0) const;
    uint64_t sequenceRevision(int layer=0) const { return sequenceVersion[(size_t)juce::jlimit(0,1,layer)].load(); }
    int sequencePosition(int lane,int layer=0) const { return lane>=0 && lane<3 && layer>=0 && layer<2 ? playheads[(size_t)(layer*3+lane)].load(std::memory_order_relaxed) : -1; }
    static const char* layerParameter(const char* id,int layer);
    int clockStatus() const { return clockState.load(std::memory_order_relaxed); }
    PlaybackSnapshot playbackInfo() const;
    void clearClip() { clipAcknowledged.store(clipRevision.load(std::memory_order_relaxed),std::memory_order_relaxed); }
    juce::String setSequenceText(int,const juce::String&,int layer=0);
    int presetCount() const;
    juce::String presetLabel(int) const;
    void loadPreset(int);
    void randomize(uint32_t seed,int lockedSections);
    void undo();
    bool canUndo() const { return !undoStack.empty(); }
    void stopArpeggiator() { keyboard.reset();resetRequested.store(true); } // Message thread.
    juce::AudioProcessorValueTreeState state;
    juce::MidiKeyboardState keyboard;
private:
    static juce::AudioProcessorValueTreeState::ParameterLayout layout();
    float value(const char*) const;
    void setParameterValue(const char*,float);
    void resetToDefaults();
    void pushUndo();
    void readParameters(bool readTransport=true);
    void midi(const juce::MidiMessage&);
    void publishPlayback(); // Audio thread, or prepare before processing starts.
    double preparedSampleRate=44100.;
    // Filled once; APVTS keeps parameter objects stable across replaceState().
    std::unordered_map<std::string_view, std::atomic<float>*> parameterValues;
    std::array<RustEngine*,2> engines{};
    RustLayerMixer* mixer=nullptr;
    std::array<RustSequenceBank*,2> canonical{};
    mutable juce::CriticalSection textMutex;
    std::array<std::array<juce::String,3>,2> texts{};
    juce::SpinLock sequenceLock;
    std::array<RustSequenceBank*,2> pending{};
    std::array<bool,2> sequenceDirty{};
    std::array<std::atomic<uint64_t>,2> sequenceVersion{};
    std::array<std::atomic<int>,6> playheads {{ -1, -1, -1, -1, -1, -1 }};
    static_assert(std::atomic<int>::is_always_lock_free);
    std::atomic<int> clockState { 3 };
    static_assert(std::atomic<float>::is_always_lock_free && std::atomic<uint32_t>::is_always_lock_free);
    std::atomic<float> outputPeak {0.f}, outputHeldPeak {0.f};
    std::array<std::atomic<uint32_t>,2> voiceActivity{};
    std::atomic<uint32_t> clipRevision {0}, clipAcknowledged {0}, playbackRevision {0};
    std::atomic<uint32_t> playbackIdleAfterMs {250};
    std::atomic<bool> resetRequested{false};
    std::vector<juce::MemoryBlock> undoStack; // Message thread only, bounded below.
    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR(RebitProcessor)
};
