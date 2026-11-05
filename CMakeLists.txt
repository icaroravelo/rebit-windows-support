cmake_minimum_required(VERSION 3.22)
project(RebitRust VERSION 0.1.0 LANGUAGES C CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_OSX_DEPLOYMENT_TARGET 11.0 CACHE STRING "Minimum macOS")
set(JUCE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/vendor/JUCE" CACHE PATH "JUCE source directory")
if(NOT EXISTS "${JUCE_PATH}/CMakeLists.txt")
    message(FATAL_ERROR "Set JUCE_PATH to a JUCE 8 checkout")
endif()
find_program(CARGO_EXECUTABLE cargo REQUIRED)
# The local build targets the current Mac architecture. Cross/universal output
# requires building Rust for each target and merging the archives explicitly.
if(CMAKE_OSX_ARCHITECTURES MATCHES ";")
    message(FATAL_ERROR "Build one architecture at a time; see README.md")
endif()
if(CMAKE_OSX_ARCHITECTURES AND NOT CMAKE_OSX_ARCHITECTURES STREQUAL CMAKE_HOST_SYSTEM_PROCESSOR)
    message(FATAL_ERROR "Rust archive uses the host architecture; omit CMAKE_OSX_ARCHITECTURES")
endif()
set(RUST_OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/cargo")
file(GLOB RUST_SOURCES CONFIGURE_DEPENDS src/*.rs)
add_custom_command(OUTPUT "${RUST_OUTPUT}/release/librebit_rust.a"
    COMMAND "${CARGO_EXECUTABLE}" build --release --lib --offline --locked
            --manifest-path "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml" --target-dir "${RUST_OUTPUT}"
    DEPENDS ${RUST_SOURCES} Cargo.toml Cargo.lock VERBATIM)
add_custom_target(rebit_rust_core DEPENDS "${RUST_OUTPUT}/release/librebit_rust.a")
add_library(rebit_rust_archive STATIC IMPORTED GLOBAL)
set_target_properties(rebit_rust_archive PROPERTIES IMPORTED_LOCATION "${RUST_OUTPUT}/release/librebit_rust.a")
add_dependencies(rebit_rust_archive rebit_rust_core)
add_subdirectory("${JUCE_PATH}" juce EXCLUDE_FROM_ALL)
juce_add_plugin(RebitRust
    COMPANY_NAME "Rebit Audio"
    BUNDLE_ID "audio.rebit.rust.instrument"
    PLUGIN_MANUFACTURER_CODE Rbit
    PLUGIN_CODE RbR1
    FORMATS AU VST3 Standalone
    PRODUCT_NAME "Rebit Rust"
    IS_SYNTH TRUE NEEDS_MIDI_INPUT TRUE NEEDS_MIDI_OUTPUT FALSE
    IS_MIDI_EFFECT FALSE EDITOR_WANTS_KEYBOARD_FOCUS FALSE
    AU_MAIN_TYPE kAudioUnitType_MusicDevice AU_SANDBOX_SAFE TRUE
    VST3_CATEGORIES Instrument Synth COPY_PLUGIN_AFTER_BUILD FALSE)
target_sources(RebitRust PRIVATE native/PluginProcessor.cpp native/PluginEditor.cpp native/SequenceCanvas.cpp native/Presets.cpp)
target_compile_definitions(RebitRust PUBLIC JUCE_WEB_BROWSER=0 JUCE_USE_CURL=0 JUCE_VST3_CAN_REPLACE_VST2=0)
target_link_libraries(RebitRust PRIVATE rebit_rust_archive juce::juce_audio_utils
    PUBLIC juce::juce_recommended_config_flags juce::juce_recommended_warning_flags)
enable_testing()
add_test(NAME rust_core COMMAND "${CARGO_EXECUTABLE}" test --offline --locked --manifest-path "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml" --target-dir "${RUST_OUTPUT}")
option(REBIT_BUILD_TOOLS "Build editor preview and VST3 host checks" ON)
if(REBIT_BUILD_TOOLS)
    add_executable(rebit_rust_au_smoke tests/au_smoke.cpp)
    target_link_libraries(rebit_rust_au_smoke PRIVATE "-framework AudioToolbox" "-framework CoreFoundation")
    add_test(NAME au_host COMMAND rebit_rust_au_smoke "$<TARGET_FILE:RebitRust_AU>")
    set_tests_properties(au_host PROPERTIES SKIP_RETURN_CODE 77)
    add_executable(rebit_rust_processor_checks tests/processor_checks.cpp)
    target_include_directories(rebit_rust_processor_checks PRIVATE native)
    target_link_libraries(rebit_rust_processor_checks PRIVATE RebitRust juce::juce_audio_utils)
    add_test(NAME native_processor COMMAND rebit_rust_processor_checks)
    add_executable(rebit_rust_preview tests/preview.cpp)
    target_include_directories(rebit_rust_preview PRIVATE native)
    target_link_libraries(rebit_rust_preview PRIVATE RebitRust juce::juce_audio_utils)
    add_executable(rebit_rust_vst3_smoke tests/vst3_smoke.cpp)
    target_compile_definitions(rebit_rust_vst3_smoke PRIVATE JUCE_PLUGINHOST_VST3=1 JUCE_WEB_BROWSER=0 JUCE_USE_CURL=0)
    target_link_libraries(rebit_rust_vst3_smoke PRIVATE juce::juce_audio_processors juce::juce_gui_basics)
    add_test(NAME vst3_host COMMAND rebit_rust_vst3_smoke "$<TARGET_BUNDLE_DIR:RebitRust_VST3>")
endif()
