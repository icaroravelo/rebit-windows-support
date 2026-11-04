#pragma once
#include <juce_audio_basics/juce_audio_basics.h>
struct TestPlayHead final : juce::AudioPlayHead {
    PositionInfo position;
    bool available=true;
    juce::Optional<PositionInfo> getPosition() const override { return available?juce::makeOptional(position):juce::nullopt; }
    void set(double ppq,double bpm,bool playing,bool looping=false,double start=0.,double end=0.) {
        position=PositionInfo{};position.setPpqPosition(ppq);position.setBpm(bpm);position.setIsPlaying(playing);
        position.setIsLooping(looping);position.setLoopPoints(LoopPoints{start,end});
        position.setTimeInSamples((juce::int64)(ppq*60./bpm*8000.));position.setTimeInSeconds(ppq*60./bpm);
    }
};
