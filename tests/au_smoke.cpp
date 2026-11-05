#include <AudioToolbox/AudioToolbox.h>
#include <dlfcn.h>
#include <cmath>
#include <iostream>
#include <stdexcept>
#include <vector>
static void check(OSStatus result,const char* operation){if(result!=noErr)throw std::runtime_error(std::string(operation)+": "+std::to_string(result));}
struct HostTiming {double beat=.1,bpm=120.,sample=10000.;bool playing=true,looping=false;};
static OSStatus beatAndTempo(void* context,Float64* beat,Float64* bpm){
    const auto& timing=*static_cast<HostTiming*>(context);if(beat)*beat=timing.beat;if(bpm)*bpm=timing.bpm;return noErr;
}
static OSStatus transportState(void* context,Boolean* playing,Boolean* changed,Float64* sample,Boolean* cycling,Float64* start,Float64* end){
    const auto& timing=*static_cast<HostTiming*>(context);if(playing)*playing=timing.playing;if(changed)*changed=false;
    if(sample)*sample=timing.sample;if(cycling)*cycling=timing.looping;if(start)*start=0.;if(end)*end=.5;return noErr;
}
static void setNamedNormalized(AudioUnit unit,const char* name,float value){
    UInt32 size=0;check(AudioUnitGetPropertyInfo(unit,kAudioUnitProperty_ParameterList,kAudioUnitScope_Global,0,&size,nullptr),"Parameter list size");
    std::vector<AudioUnitParameterID> ids(size/sizeof(AudioUnitParameterID));
    check(AudioUnitGetProperty(unit,kAudioUnitProperty_ParameterList,kAudioUnitScope_Global,0,ids.data(),&size),"Parameter list");
    for(auto id:ids){
        AudioUnitParameterInfo info{};size=sizeof(info);check(AudioUnitGetProperty(unit,kAudioUnitProperty_ParameterInfo,kAudioUnitScope_Global,id,&info,&size),"Parameter info");
        char text[256]{};
        if(info.cfNameString)CFStringGetCString(info.cfNameString,text,sizeof(text),kCFStringEncodingUTF8);
        else std::snprintf(text,sizeof(text),"%s",info.name);
        if(info.cfNameString&&(info.flags&kAudioUnitParameterFlag_CFNameRelease))CFRelease(info.cfNameString);
        if(std::string(text)==name){check(AudioUnitSetParameter(unit,id,kAudioUnitScope_Global,0,info.minValue+value*(info.maxValue-info.minValue),0),"Set clock parameter");return;}
    }
    throw std::runtime_error(std::string("Parameter missing: ")+name);
}
int main(int argc,char** argv){
    if(argc!=2)return 2;
    try{
        // Register just inside this process; no installation or system registry edits.
        auto* library=dlopen(argv[1],RTLD_NOW|RTLD_LOCAL);
        if(!library)throw std::runtime_error(dlerror());
        auto factory=reinterpret_cast<AudioComponentFactoryFunction>(dlsym(library,"Rebit_RustAUFactory"));
        if(!factory)throw std::runtime_error("Missing AU factory");
        AudioComponentDescription any{};
        if(!AudioComponentFindNext(nullptr,&any)){
            std::cout<<"SKIP: macOS AudioComponent registry is unavailable (no system units visible). Run outside the restricted session.\n";
            return 77;
        }
        AudioComponentDescription desc{'aumu','RbR1','Rbit',kAudioComponentFlag_SandboxSafe,0};
        auto component=AudioComponentRegister(&desc,CFSTR("Rebit Audio: Rebit Rust test"),0x100,factory);
        if(!component)throw std::runtime_error("AU registration failed");
        AudioUnit unit=nullptr;check(AudioComponentInstanceNew(component,&unit),"Instantiate");
        AudioStreamBasicDescription format{};format.mSampleRate=48000;format.mFormatID=kAudioFormatLinearPCM;
        format.mFormatFlags=kAudioFormatFlagIsFloat|kAudioFormatFlagIsPacked|kAudioFormatFlagIsNonInterleaved;
        format.mBytesPerPacket=4;format.mFramesPerPacket=1;format.mBytesPerFrame=4;format.mChannelsPerFrame=2;format.mBitsPerChannel=32;
        check(AudioUnitSetProperty(unit,kAudioUnitProperty_StreamFormat,kAudioUnitScope_Output,0,&format,sizeof(format)),"Stream format");
        UInt32 maxFrames=128;check(AudioUnitSetProperty(unit,kAudioUnitProperty_MaximumFramesPerSlice,kAudioUnitScope_Global,0,&maxFrames,sizeof(maxFrames)),"Block size");
        check(AudioUnitInitialize(unit),"Initialize");
        float left[128]{},right[128]{};
        struct {UInt32 count;AudioBuffer buffers[2];} storage{2,{{1,sizeof(left),left},{1,sizeof(right),right}}};
        auto* buffers=reinterpret_cast<AudioBufferList*>(&storage);
        AudioTimeStamp time{};time.mFlags=kAudioTimeStampSampleTimeValid;
        AudioUnitRenderActionFlags flags=0;
        check(MusicDeviceMIDIEvent(unit,0x90,69,127,0),"Note on");
        check(AudioUnitRender(unit,&flags,&time,0,128,buffers),"Render");
        if(std::abs(left[0]+0.5f)>1e-7f)throw std::runtime_error("Unexpected first sample");
        for(int i=0;i<128;++i)if(!std::isfinite(left[i])||std::abs(left[i]-right[i])>1e-7f)throw std::runtime_error("Invalid stereo render");
        CFPropertyListRef state=nullptr;UInt32 size=sizeof(state);
        check(AudioUnitGetProperty(unit,kAudioUnitProperty_ClassInfo,kAudioUnitScope_Global,0,&state,&size),"Save state");
        if(!state)throw std::runtime_error("Empty state");
        check(AudioUnitSetProperty(unit,kAudioUnitProperty_ClassInfo,kAudioUnitScope_Global,0,&state,sizeof(state)),"Restore state");CFRelease(state);
        time.mSampleTime=128;flags=0;check(AudioUnitRender(unit,&flags,&time,0,128,buffers),"Render restored state");
        check(MusicDeviceMIDIEvent(unit,0x90,69,127,0),"Restored note on");
        time.mSampleTime=256;flags=0;check(AudioUnitRender(unit,&flags,&time,0,128,buffers),"Restored note render");
        if(std::abs(left[0]+0.5f)>1e-7f)throw std::runtime_error("State restore changed sound");
        setNamedNormalized(unit,"Gain",0.f);
        time.mSampleTime=384;flags=0;check(AudioUnitRender(unit,&flags,&time,0,128,buffers),"Render live gain fade");
        if(std::abs(left[0])<.49f||std::abs(std::abs(left[127])-(.5f*(1.f-128.f/240.f)))>1e-6f)
            throw std::runtime_error("AU gain automation did not ramp");
        time.mSampleTime=512;flags=0;check(AudioUnitRender(unit,&flags,&time,0,128,buffers),"Finish live gain fade");
        for(int i=111;i<128;++i)if(left[i]!=0.f)throw std::runtime_error("AU gain fade did not settle in five ms");
        setNamedNormalized(unit,"Gain",.5f);
        check(MusicDeviceMIDIEvent(unit,0xb0,120,0,0),"Clear before layer test");
        time.mSampleTime=640;flags=0;check(AudioUnitRender(unit,&flags,&time,0,128,buffers),"Clear render");
        time.mSampleTime=768;flags=0;check(AudioUnitRender(unit,&flags,&time,0,128,buffers),"Settle gain before layer test");
        setNamedNormalized(unit,"Layer B enabled",1);setNamedNormalized(unit,"Layer B level",1);
        setNamedNormalized(unit,"Layer A pan",0);setNamedNormalized(unit,"Layer B pan",1);setNamedNormalized(unit,"Layer B transpose",.5f);
        check(MusicDeviceMIDIEvent(unit,0x90,0,127,0),"Layer note on");
        time.mSampleTime=896;flags=0;check(AudioUnitRender(unit,&flags,&time,0,128,buffers),"Layer stereo render");
        if(std::abs(left[0]+.5f)>1e-6f||std::abs(right[0]+8.f/15.f)>1e-6f)throw std::runtime_error("AU layer stereo routing differs");
        setNamedNormalized(unit,"Layer B enabled",0);setNamedNormalized(unit,"Layer A pan",.5f);
        check(AudioUnitUninitialize(unit),"Uninitialize before timing checks");
        format.mSampleRate=8000;maxFrames=2400;
        check(AudioUnitSetProperty(unit,kAudioUnitProperty_StreamFormat,kAudioUnitScope_Output,0,&format,sizeof(format)),"Timing stream format");
        check(AudioUnitSetProperty(unit,kAudioUnitProperty_MaximumFramesPerSlice,kAudioUnitScope_Global,0,&maxFrames,sizeof(maxFrames)),"Timing block size");
        HostTiming timing;HostCallbackInfo callbacks{};callbacks.hostUserData=&timing;callbacks.beatAndTempoProc=beatAndTempo;callbacks.transportStateProc=transportState;
        check(AudioUnitSetProperty(unit,kAudioUnitProperty_HostCallbacks,kAudioUnitScope_Global,0,&callbacks,sizeof(callbacks)),"Host callbacks");
        setNamedNormalized(unit,"Arpeggiator",1);setNamedNormalized(unit,"Host sync",1);setNamedNormalized(unit,"Arp gate",45.f/95.f);
        check(AudioUnitInitialize(unit),"Initialize timing test");
        float timingLeft[2400]{},timingRight[2400]{};
        struct {UInt32 count;AudioBuffer buffers[2];} timingStorage{2,{{1,sizeof(timingLeft),timingLeft},{1,sizeof(timingRight),timingRight}}};
        auto renderTiming=[&](UInt32 frames){
            timingStorage.buffers[0].mDataByteSize=frames*sizeof(float);timingStorage.buffers[1].mDataByteSize=frames*sizeof(float);
            AudioTimeStamp stamp{};stamp.mFlags=kAudioTimeStampSampleTimeValid;stamp.mSampleTime=timing.sample;
            AudioUnitRenderActionFlags action=0;check(AudioUnitRender(unit,&action,&stamp,0,frames,reinterpret_cast<AudioBufferList*>(&timingStorage)),"Render host timing");
            timing.sample+=frames;
        };
        check(MusicDeviceMIDIEvent(unit,0x90,60,127,17),"Timing note on");check(MusicDeviceMIDIEvent(unit,0x90,67,127,17),"Timing chord note");renderTiming(1200);
        for(int i=0;i<1200;++i){const bool on=i>=600&&i<1100;if(on?std::abs(timingLeft[i])<.49f:std::abs(timingLeft[i])>1e-7f)throw std::runtime_error("AU PPQ/gate alignment differs at sample "+std::to_string(i)+": "+std::to_string(timingLeft[i]));}
        timing.beat=.4;timing.playing=false;renderTiming(1200);
        for(int i=0;i<1200;++i)if(std::abs(timingLeft[i])>1e-7f)throw std::runtime_error("AU stop did not suspend arp");
        timing.beat=4.;timing.playing=true;renderTiming(1200);
        if(std::abs(timingLeft[0]+.5f)>1e-7f)throw std::runtime_error("AU restart lost held chord");
        timing.beat=0.;timing.looping=true;renderTiming(2400);
        for(int i=0;i<400;++i)if(std::abs(timingLeft[i]-timingLeft[2000+i])>1e-7f)throw std::runtime_error("AU loop points did not reach engine");
        check(AudioUnitUninitialize(unit),"Uninitialize before colour check");
        setNamedNormalized(unit,"Arpeggiator",0);setNamedNormalized(unit,"Host sync",0);setNamedNormalized(unit,"Layer B enabled",0);
        setNamedNormalized(unit,"Layer A sound mode",1);setNamedNormalized(unit,"Effects enabled",1);setNamedNormalized(unit,"Saturation drive",.5f);
        setNamedNormalized(unit,"Bit depth",.2f);setNamedNormalized(unit,"Rate divider",7.f/31.f);setNamedNormalized(unit,"Effects mix",1);
        check(AudioUnitInitialize(unit),"Initialize colour check");
        check(MusicDeviceMIDIEvent(unit,0x90,0,127,0),"Colour note on");renderTiming(128);
        for(int i=0;i<8;++i)if(std::abs(timingLeft[i])>1e-7f)throw std::runtime_error("AU Clean edge/sample hold differs");
        if(std::abs(timingLeft[8])<.5f)throw std::runtime_error("AU saturation did not reach output");
        for(int i=8;i<16;++i)if(std::abs(timingLeft[i]-timingLeft[8])>1e-7f||std::abs(timingRight[i]-timingLeft[i])>1e-7f)throw std::runtime_error("AU sample hold/stereo alignment differs");
        check(MusicDeviceMIDIEvent(unit,0xb0,120,0,3),"Colour All Sound Off");renderTiming(128);
        for(int i=3;i<128;++i)if(std::abs(timingLeft[i])>1e-7f)throw std::runtime_error("AU panic left effect history playing");
        check(AudioUnitUninitialize(unit),"Uninitialize");check(AudioComponentInstanceDispose(unit),"Dispose");
        // Keep the registered factory's library loaded for process lifetime.
        std::cout<<"AU: direct registration, instantiation, MIDI, stereo render, state recall, host PPQ/start-stop/loops passed\n";
    }catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}
}
