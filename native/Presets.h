#pragma once
#include <array>
#include <string>
#include <vector>

struct FactoryParameter {
    const char* id;
    float value;
};
struct FactoryPreset {
    std::string name;
    std::string category;
    std::vector<FactoryParameter> parameters;
    std::array<std::string, 3> sequenceA { "", "", "" };
    std::array<std::string, 3> sequenceB { "", "", "" };
    std::array<bool, 3> lanesA { false, false, false };
    std::array<bool, 3> lanesB { false, false, false };
};

const std::vector<FactoryPreset>& factoryPresets();
