#include "Presets.h"

namespace {
FactoryPreset preset (const char* name, const char* category,
                      std::initializer_list<FactoryParameter> parameters)
{
    FactoryPreset result;
    result.name = name;
    result.category = category;
    result.parameters.assign (parameters.begin(), parameters.end());
    return result;
}
}

const std::vector<FactoryPreset>& factoryPresets()
{
    static const std::vector<FactoryPreset> presets = [] {
        std::vector<FactoryPreset> list;
        list.push_back (preset ("Init", "Init", {}));

        list.push_back (preset ("Sub Pulse", "Bass", {
            { "osc", 0 }, { "duty", 2 }, { "maxPoly", 1 }, { "behavior", 0 }, { "gain", 0.55f },
            { "attack", 0.002f }, { "decay", 0.12f }, { "suslevel", 0.8f }, { "release", 0.08f },
            { "portamento", 0.03f }, { "layerBEnabled", 1 }, { "layerAGain", 0.8f },
            { "layerBGain", 0.45f }, { "layerBTranspose", -12 }, { "layerBOsc", 1 },
            { "layerBAttack", 0.004f }, { "layerBDecay", 0.15f }, { "layerBSustain", 0.85f },
            { "layerBRelease", 0.1f }, { "fxEnabled", 0 }
        }));

        list.push_back (preset ("Rubber Bass", "Bass", {
            { "osc", 0 }, { "duty", 1 }, { "maxPoly", 1 }, { "behavior", 0 }, { "gain", 0.5f },
            { "attack", 0.004f }, { "decay", 0.18f }, { "suslevel", 0.45f }, { "release", 0.12f },
            { "portamento", 0.06f }, { "vibratoDepth", 0.08f }, { "vibratoRate", 0.16f },
            { "vibratoDelay", 0.2f }, { "layerBEnabled", 0 },
            { "mod1Source", 5 }, { "mod1Dest", 2 }, { "mod1Amount", 35 },
            { "lfo1Shape", 0 }, { "lfo1Rate", 1.4f }, { "fxEnabled", 0 }
        }));

        list.push_back (preset ("Wobble", "Bass", {
            { "osc", 0 }, { "duty", 2 }, { "maxPoly", 1 }, { "behavior", 0 }, { "gain", 0.45f },
            { "attack", 0.003f }, { "decay", 0.2f }, { "suslevel", 0.85f }, { "release", 0.12f },
            { "layerBEnabled", 0 }, { "mod1Source", 5 }, { "mod1Dest", 2 }, { "mod1Amount", 70 },
            { "lfo1Shape", 1 }, { "lfo1Rate", 3.8f }, { "fxEnabled", 1 }, { "fxDrive", 8 },
            { "fxBits", 12 }, { "fxRate", 2 }, { "fxMix", 60 }
        }));

        list.push_back (preset ("Arcade Lead", "Lead", {
            { "osc", 0 }, { "duty", 1 }, { "maxPoly", 8 }, { "behavior", 0 }, { "gain", 0.35f },
            { "attack", 0.005f }, { "decay", 0.18f }, { "suslevel", 0.7f }, { "release", 0.2f },
            { "vibratoRate", 0.19f }, { "vibratoDepth", 0.18f }, { "vibratoDelay", 0.22f },
            { "layerBEnabled", 1 }, { "layerAGain", 0.7f }, { "layerBGain", 0.35f },
            { "layerAPan", -0.25f }, { "layerBPan", 0.25f }, { "layerBDetune", 7 },
            { "layerBOsc", 1 }, { "layerBAttack", 0.006f }, { "layerBDecay", 0.2f },
            { "layerBSustain", 0.75f }, { "layerBRelease", 0.25f },
            { "fxEnabled", 1 }, { "fxDrive", 3 }, { "fxMix", 80 }
        }));

        list.push_back (preset ("Glide Lead", "Lead", {
            { "osc", 0 }, { "duty", 0 }, { "maxPoly", 1 }, { "behavior", 0 }, { "gain", 0.45f },
            { "attack", 0.01f }, { "decay", 0.15f }, { "suslevel", 0.8f }, { "release", 0.2f },
            { "portamento", 0.12f }, { "vibratoRate", 0.17f }, { "vibratoDepth", 0.28f },
            { "vibratoDelay", 0.25f }, { "layerBEnabled", 0 }, { "fxEnabled", 0 }
        }));

        auto hat = preset ("Noise Hat", "Percussion", {
            { "osc", 2 }, { "noiseAlgorithm_raw", 1 }, { "maxPoly", 8 }, { "behavior", 3 },
            { "gain", 0.5f }, { "attack", 0 }, { "decay", 0.035f }, { "suslevel", 0 },
            { "release", 0.02f }, { "layerBEnabled", 0 }, { "fxEnabled", 1 },
            { "fxBits", 8 }, { "fxRate", 3 }, { "fxMix", 55 }
        });
        hat.sequenceA[0] = "15,8,15,4[15,8]|0";
        hat.lanesA[0] = true;
        list.push_back (std::move (hat));

        list.push_back (preset ("Chip Kick", "Percussion", {
            { "osc", 1 }, { "maxPoly", 1 }, { "behavior", 3 }, { "gain", 0.7f },
            { "attack", 0 }, { "decay", 0.09f }, { "suslevel", 0 }, { "release", 0.02f },
            { "sweepInitialPitch", 24 }, { "sweepTime", 0.045f }, { "layerBEnabled", 0 },
            { "fxEnabled", 1 }, { "fxDrive", 6 }, { "fxBits", 16 }, { "fxRate", 2 }, { "fxMix", 70 }
        }));

        list.push_back (preset ("Coin", "Game", {
            { "osc", 0 }, { "duty", 2 }, { "maxPoly", 1 }, { "behavior", 3 }, { "gain", 0.4f },
            { "attack", 0 }, { "decay", 0.07f }, { "suslevel", 0 }, { "release", 0.03f },
            { "sweepInitialPitch", 24 }, { "sweepTime", 0.07f }, { "layerBEnabled", 0 },
            { "fxEnabled", 1 }, { "fxMix", 60 }
        }));

        list.push_back (preset ("Laser", "Game", {
            { "osc", 0 }, { "duty", 0 }, { "maxPoly", 1 }, { "behavior", 3 }, { "gain", 0.4f },
            { "attack", 0 }, { "decay", 0.12f }, { "suslevel", 0 }, { "release", 0.05f },
            { "sweepInitialPitch", -24 }, { "sweepTime", 0.1f }, { "layerBEnabled", 0 },
            { "fxEnabled", 1 }, { "fxDrive", 10 }, { "fxBits", 12 }, { "fxRate", 2 }, { "fxMix", 65 }
        }));

        auto power = preset ("Power Up", "Game", {
            { "osc", 0 }, { "duty", 1 }, { "maxPoly", 4 }, { "gain", 0.3f },
            { "attack", 0.003f }, { "decay", 0.12f }, { "suslevel", 0.5f }, { "release", 0.15f },
            { "arpEnabled", 1 }, { "arpPattern", 0 }, { "arpRate", 3 }, { "arpOctaves", 2 },
            { "arpGate", 70 }, { "layerBEnabled", 1 }, { "layerBGain", 0.25f },
            { "layerBTranspose", 12 }, { "layerBOsc", 1 }, { "fxEnabled", 1 },
            { "fxDrive", 4 }, { "fxBits", 16 }, { "fxRate", 2 }, { "fxMix", 50 }
        });
        power.sequenceA[1] = "0,4,7,12[12,7,4]|0";
        power.lanesA[1] = true;
        list.push_back (std::move (power));

        list.push_back (preset ("Wide Layers", "Pad", {
            { "osc", 0 }, { "duty", 1 }, { "maxPoly", 8 }, { "gain", 0.5f },
            { "attack", 0.6f }, { "decay", 0.8f }, { "suslevel", 0.85f }, { "release", 1.4f },
            { "layerBEnabled", 1 }, { "layerAGain", 0.55f }, { "layerBGain", 0.5f },
            { "layerAPan", -0.35f }, { "layerBPan", 0.35f }, { "layerBDetune", 9 },
            { "layerBOsc", 1 }, { "layerBDuty", 1 }, { "layerBAttack", 0.9f },
            { "layerBDecay", 1.2f }, { "layerBSustain", 0.9f }, { "layerBRelease", 1.6f },
            { "vibratoRate", 0.12f }, { "vibratoDepth", 0.12f }, { "vibratoDelay", 0.4f },
            { "fxEnabled", 1 }, { "fxDrive", 4 }, { "fxBits", 16 }, { "fxRate", 2 },
            { "fxMix", 40 }, { "mod1Source", 5 }, { "mod1Dest", 2 }, { "mod1Amount", 20 },
            { "lfo1Shape", 0 }, { "lfo1Rate", 0.35f }
        }));

        return list;
    }();
    return presets;
}
