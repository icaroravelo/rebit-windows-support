#include "PluginEditor.h"
#include "SequenceCanvas.h"
#include <cmath>

namespace
{
const juce::Colour background { 0xff0b1017 }, panel { 0xff121b25 }, inset { 0xff0b121b };
const juce::Colour ink { 0xffedf2ef }, muted { 0xff91a3b3 }, mint { 0xffc4f575 };
const juce::Colour cyan { 0xff63dfe5 }, violet { 0xffb5a0f5 }, amber { 0xffffb782 };
const juce::Colour line { 0xff293744 };
constexpr float pi = juce::MathConstants<float>::pi;
constexpr int accentColourId = 0x234567;

juce::Font type (float size, bool mono = false, bool bold = false)
{
    return juce::Font (juce::FontOptions (mono ? juce::Font::getDefaultMonospacedFontName()
                                             : juce::Font::getDefaultSansSerifFontName(),
                                         size, bold ? juce::Font::bold : juce::Font::plain));
}
void caption (juce::Graphics& g, const juce::String& text, juce::Rectangle<int> bounds,
              juce::Colour colour = muted, float size = 11.0f, bool mono = false)
{
    g.setColour (colour);
    g.setFont (type (size, mono));
    g.drawText (text, bounds, juce::Justification::centredLeft);
}
void bloom (juce::Graphics& g, const juce::Path& path, juce::Colour colour, float width = 1.5f)
{
    for (const auto spread : { 12.0f, 7.0f, 3.5f })
    {
        g.setColour (colour.withAlpha (0.035f));
        g.strokePath (path, juce::PathStrokeType (width + spread));
    }
    g.setColour (colour);
    g.strokePath (path, juce::PathStrokeType (width));
}
void panelShape (juce::Graphics& g, juce::Rectangle<float> bounds)
{
    g.setColour (juce::Colours::black.withAlpha (0.24f));
    g.fillRoundedRectangle (bounds.translated (0, 3), 10);
    g.setGradientFill (juce::ColourGradient (panel.brighter (0.035f), bounds.getTopLeft(),
                                            panel, bounds.getBottomRight(), false));
    g.fillRoundedRectangle (bounds, 10);
    g.setColour (line.withAlpha (0.7f));
    g.drawRoundedRectangle (bounds.reduced (0.5f), 10, 1);
}
void section (juce::Graphics& g, juce::Rectangle<float> bounds, const juce::String& number,
              const juce::String& name, juce::Colour accent)
{
    panelShape (g, bounds);
    const int x = (int) bounds.getX(), y = (int) bounds.getY();
    g.setColour (accent);
    g.fillRect (x + 16, y + 17, 3, 3);
    g.fillRect (x + 20, y + 21, 3, 3);
    caption (g, number, { x + 32, y + 10, 28, 22 }, accent, 10, true);
    caption (g, name, { x + 65, y + 10, 300, 22 }, ink, 11.5f);
}
void pixelLogo (juce::Graphics& g, float x, float y, float cell)
{
    // Five original 5 x 7 glyphs, drawn as vectors so the mark stays crisp at HiDPI.
    constexpr const char* glyphs[] = {
        "11110100011000111110101001001010001", // R
        "11111100001000011110100001000011111", // E
        "11110100011000111110100011000111110", // B
        "11111001000010000100001000010011111", // I
        "11111001000010000100001000010000100"  // T
    };
    juce::Path mark;
    for (int c = 0; c < 5; ++c)
        for (int row = 0; row < 7; ++row)
            for (int col = 0; col < 5; ++col)
                if (glyphs[c][row * 5 + col] == '1')
                    mark.addRectangle (x + (float) (c * 6 + col) * cell,
                                       y + (float) row * cell, cell - 0.35f, cell - 0.35f);
    g.setColour (mint.withAlpha (0.035f));
    g.fillPath (mark, juce::AffineTransform::translation (0, 3));
    g.setColour (ink);
    g.fillPath (mark);
}
void pixelSpark (juce::Graphics& g, int x, int y, juce::Colour colour)
{
    constexpr int rows[] = { 0b00100, 0b01110, 0b11011, 0b01110, 0b00100 };
    g.setColour (colour);
    for (int row = 0; row < 5; ++row)
        for (int col = 0; col < 5; ++col)
            if ((rows[row] & (1 << col)) != 0) g.fillRect (x + col * 5, y + row * 5, 4, 4);
}
}

class RebitEditor::RebitLookAndFeel final : public juce::LookAndFeel_V4
{
public:
    RebitLookAndFeel()
    {
        setColour (juce::Slider::textBoxTextColourId, ink);
        setColour (juce::Slider::textBoxBackgroundColourId, juce::Colours::transparentBlack);
        setColour (juce::Slider::textBoxOutlineColourId, juce::Colours::transparentBlack);
        setColour (juce::Slider::textBoxHighlightColourId, cyan.withAlpha (0.2f));
        setColour (juce::ComboBox::backgroundColourId, inset);
        setColour (juce::ComboBox::textColourId, ink);
        setColour (juce::ComboBox::outlineColourId, line);
        setColour (juce::PopupMenu::backgroundColourId, panel);
        setColour (juce::PopupMenu::textColourId, ink);
        setColour (juce::PopupMenu::highlightedBackgroundColourId, mint);
        setColour (juce::PopupMenu::highlightedTextColourId, background);
        setColour (juce::ToggleButton::textColourId, muted);
        setColour (juce::TextEditor::backgroundColourId, inset);
        setColour (juce::TextEditor::textColourId, ink);
        setColour (juce::TextEditor::outlineColourId, line);
        setColour (juce::TextEditor::focusedOutlineColourId, cyan);
        setColour (juce::TextEditor::highlightColourId, cyan.withAlpha (0.22f));
        setColour (juce::CaretComponent::caretColourId, mint);
        setColour (juce::TooltipWindow::backgroundColourId, panel);
        setColour (juce::TooltipWindow::textColourId, ink);
        setColour (juce::TooltipWindow::outlineColourId, line);
    }
    void drawRotarySlider (juce::Graphics& g, int x, int y, int w, int h, float position,
                           float start, float end, juce::Slider& slider) override
    {
        const auto area = juce::Rectangle<float> ((float) x, (float) y, (float) w, (float) h);
        const float radius = juce::jmin (area.getWidth(), area.getHeight()) * 0.5f - 7;
        if (radius < 3) return;
        const float cx = area.getCentreX(), cy = area.getCentreY();
        const bool enabled = slider.isEnabled();
        const auto accent = enabled ? slider.findColour (accentColourId) : muted.withAlpha (0.22f);
        const auto angle = start + position * (end - start);
        for (int tick = 0; tick <= 24; ++tick)
        {
            const float a = start + (float) tick / 24 * (end - start);
            g.setColour ((float) tick / 24 <= position ? accent : line);
            const float outer = radius + 4, inner = radius + 1;
            g.drawLine (cx + std::sin (a) * inner, cy - std::cos (a) * inner,
                        cx + std::sin (a) * outer, cy - std::cos (a) * outer, 1.7f);
        }
        juce::Path track, value;
        track.addCentredArc (cx, cy, radius - 2, radius - 2, 0, start, end, true);
        value.addCentredArc (cx, cy, radius - 2, radius - 2, 0, start, angle, true);
        g.setColour (line.withAlpha (0.6f));
        g.strokePath (track, juce::PathStrokeType (1.6f));
        bloom (g, value, accent.withMultipliedAlpha (0.9f), 2);
        const float body = radius - 8;
        g.setColour (juce::Colours::black.withAlpha (0.4f));
        g.fillEllipse (cx - body, cy - body + 2, body * 2, body * 2);
        g.setGradientFill (juce::ColourGradient (juce::Colour (0xff293745), cx, cy - body,
                                                juce::Colour (0xff121b25), cx, cy + body, false));
        g.fillEllipse (cx - body, cy - body, body * 2, body * 2);
        g.setColour (line.brighter (0.13f));
        g.drawEllipse (cx - body, cy - body, body * 2, body * 2, 0.8f);
        if (slider.isMouseOverOrDragging() || slider.hasKeyboardFocus (false))
        {
            g.setColour (accent.withAlpha (0.3f));
            g.drawEllipse (cx - body + 2, cy - body + 2, body * 2 - 4, body * 2 - 4, 1);
        }
        g.setColour (enabled ? ink : muted.withAlpha (0.4f));
        g.drawLine (cx + std::sin (angle) * (body * 0.42f), cy - std::cos (angle) * (body * 0.42f),
                    cx + std::sin (angle) * (body - 4), cy - std::cos (angle) * (body - 4), 2.2f);
    }
    juce::Label* createSliderTextBox (juce::Slider& slider) override
    {
        auto* label = juce::LookAndFeel_V4::createSliderTextBox (slider);
        label->setFont (type (12, true));
        label->setColour (juce::Label::outlineColourId, juce::Colours::transparentBlack);
        label->setColour (juce::TextEditor::outlineColourId, cyan);
        return label;
    }
    void drawComboBox (juce::Graphics& g, int width, int height, bool, int, int, int, int,
                       juce::ComboBox& box) override
    {
        const auto r = juce::Rectangle<float> (0, 0, (float) width, (float) height).reduced (0.5f);
        g.setColour (inset); g.fillRoundedRectangle (r, 4);
        const auto accent = box.findColour (accentColourId);
        g.setColour (box.hasKeyboardFocus (true) || box.isMouseOver() ? accent.withAlpha (0.7f) : line);
        g.drawRoundedRectangle (r, 4, 1);
        g.setColour (box.isEnabled() ? accent : muted.withAlpha (0.3f));
        const float px = (float) width - 18, py = (float) height * 0.5f;
        g.fillRect (px - 3, py - 2, 2.0f, 2.0f);
        g.fillRect (px - 1, py, 2.0f, 2.0f);
        g.fillRect (px + 1, py - 2, 2.0f, 2.0f);
    }
    juce::Font getComboBoxFont (juce::ComboBox&) override { return type (12); }
    void positionComboBoxText (juce::ComboBox& box, juce::Label& label) override
    {
        label.setBounds (11, 0, box.getWidth() - 35, box.getHeight());
        label.setFont (getComboBoxFont (box));
    }
    void drawToggleButton (juce::Graphics& g, juce::ToggleButton& button, bool hover, bool) override
    {
        const auto accent = button.findColour (accentColourId);
        const bool on = button.getToggleState();
        const float y = (float) button.getHeight() * 0.5f - 6;
        g.setColour (on ? accent.withAlpha (0.12f) : inset);
        g.fillRoundedRectangle (0, y, 24, 12, 3);
        g.setColour (on ? accent.withAlpha (0.6f) : (hover ? muted : line));
        g.drawRoundedRectangle (0.5f, y + 0.5f, 23, 11, 3, 1);
        g.setColour (on ? accent : muted.withAlpha (0.6f));
        g.fillRect (on ? 15.0f : 4.0f, y + 3, 5.0f, 6.0f);
        caption (g, button.getButtonText(), { 33, 0, button.getWidth() - 33, button.getHeight() },
                 on ? ink : muted, 11.5f);
    }
    void drawButtonBackground (juce::Graphics& g, juce::Button& button, const juce::Colour&, bool hover, bool down) override
    {
        if ((bool) button.getProperties()["sequenceSmall"])
        {
            const auto c=button.findColour(accentColourId);
            const auto bounds=button.getLocalBounds().toFloat().reduced(.5f);
            g.setColour(button.getToggleState()?c.withAlpha(.12f):inset);
            g.fillRoundedRectangle(bounds,3);
            g.setColour(button.getToggleState()?c.withAlpha(.6f):hover?muted:line);
            g.drawRoundedRectangle(bounds,3,1);return;
        }
        auto bounds = button.getLocalBounds().toFloat().reduced (0.5f);
        g.setColour (down ? mint.darker (0.15f) : hover ? mint.brighter (0.12f) : mint);
        g.fillRoundedRectangle (bounds, 4);
        g.setColour (juce::Colours::white.withAlpha (0.2f));
        g.drawRoundedRectangle (bounds, 4, 1);
    }
    void drawButtonText (juce::Graphics& g, juce::TextButton& button, bool, bool) override
    {
        if ((bool) button.getProperties()["sequenceSmall"])
        {
            g.setColour(button.isEnabled()?(button.getToggleState()?button.findColour(accentColourId):ink):muted.withAlpha(.35f));
            g.setFont(type(10,true));g.drawText(button.getButtonText(),button.getLocalBounds(),juce::Justification::centred);return;
        }
        g.setColour (background); g.setFont (type (12, true, true));
        g.drawText (button.getButtonText(), button.getLocalBounds(), juce::Justification::centred);
    }
    void drawTextEditorOutline (juce::Graphics& g, int w, int h, juce::TextEditor& editor) override
    {
        const auto colour = editor.findColour (juce::TextEditor::outlineColourId);
        g.setColour (editor.hasKeyboardFocus (true) ? cyan : colour);
        g.drawRoundedRectangle (0.5f, 0.5f, (float) w - 1, (float) h - 1, 3, 1);
    }
};

struct RebitEditor::Keyboard final : public juce::MidiKeyboardComponent
{
    explicit Keyboard (juce::MidiKeyboardState& state) : MidiKeyboardComponent (state, horizontalKeyboard) {}
    void drawUpDownButton (juce::Graphics& g, int w, int h, bool over, bool down, bool up) override
    {
        g.setColour (over ? panel.brighter (0.13f) : panel); g.fillAll();
        g.setColour (down ? mint : over ? cyan : muted);
        const float cx = (float) w * 0.5f, cy = (float) h * 0.5f;
        juce::Path arrow; const float direction = up ? 1.0f : -1.0f;
        arrow.startNewSubPath (cx - direction * 2, cy - 4);
        arrow.lineTo (cx + direction * 2, cy); arrow.lineTo (cx - direction * 2, cy + 4);
        g.strokePath (arrow, juce::PathStrokeType (1.5f));
    }
    void drawWhiteNote (int note, juce::Graphics& g, juce::Rectangle<float> area,
                        bool down, bool over, juce::Colour, juce::Colour) override
    {
        auto r = area.reduced (1, 1);
        g.setGradientFill (juce::ColourGradient (down ? mint : over ? juce::Colour (0xffe0e9e7) : juce::Colour (0xffa7b7bc),
                                                r.getTopLeft(), down ? mint.darker (0.15f) : juce::Colour (0xffdae2df), r.getBottomLeft(), false));
        g.fillRoundedRectangle (r, 2);
        g.setColour (juce::Colours::black.withAlpha (0.15f));
        g.fillRect (r.removeFromBottom (3));
        if (note % 12 == 0)
        {
            g.setColour (juce::Colour (0xff384955)); g.setFont (type (9, true));
            g.drawText (juce::MidiMessage::getMidiNoteName (note, true, true, 3), area.toNearestInt().withTrimmedTop (30), juce::Justification::centred);
        }
    }
    void drawBlackNote (int, juce::Graphics& g, juce::Rectangle<float> area,
                        bool down, bool over, juce::Colour) override
    {
        auto r = area.reduced (0.5f, 0);
        g.setColour (juce::Colours::black.withAlpha (0.45f));
        g.fillRoundedRectangle (r.translated (1, 2), 2);
        g.setGradientFill (juce::ColourGradient (down ? cyan.darker (0.35f) : over ? line.brighter (0.2f) : juce::Colour (0xff253541),
                                                r.getTopLeft(), down ? cyan : inset, r.getBottomLeft(), false));
        g.fillRoundedRectangle (r, 2);
        g.setColour (down ? cyan : line.brighter (0.2f));
        g.fillRect (r.getX() + 3, r.getBottom() - 5, r.getWidth() - 6, 2.0f);
    }
};

struct RebitEditor::Knob final : public juce::Component
{
    juce::Label title;
    juce::Slider slider;
    std::unique_ptr<juce::AudioProcessorValueTreeState::SliderAttachment> attachment;
    Knob (juce::AudioProcessorValueTreeState& state, const juce::String& id,
          const juce::String& name, const juce::String& suffix, juce::Colour accent)
    {
        title.setText (name, juce::dontSendNotification);
        title.setJustificationType (juce::Justification::centred);
        title.setColour (juce::Label::textColourId, muted);
        title.setFont (type (10));
        slider.setName (name);
        slider.setTooltip (name + ": drag to adjust, double-click to reset, or click the value to type.");
        slider.setSliderStyle (juce::Slider::RotaryHorizontalVerticalDrag);
        slider.setRotaryParameters (pi * 1.25f, pi * 2.75f, true);
        slider.setTextBoxStyle (juce::Slider::TextBoxBelow, false, 100, 20);
        slider.setTextValueSuffix (suffix);
        slider.setColour (accentColourId, accent);
        slider.setMouseDragSensitivity (180);
        auto* parameter = state.getParameter (id);
        if (parameter != nullptr) slider.setDoubleClickReturnValue (true, parameter->convertFrom0to1 (parameter->getDefaultValue()));
        addAndMakeVisible (title); addAndMakeVisible (slider);
        attachment = std::make_unique<juce::AudioProcessorValueTreeState::SliderAttachment> (state, id, slider);
        slider.setNumDecimalPlacesToDisplay (id == "maxPoly" || id == "bendRange" || id == "sweepInitialPitch" || id == "arpGate" || id == "arpSwing" ? 0 : id == "gain" || id == "suslevel" ? 2 : 3);
        slider.textFromValueFunction = nullptr; slider.updateText();
    }
    void resized() override
    {
        auto r = getLocalBounds(); title.setBounds (r.removeFromTop (17)); slider.setBounds (r);
    }
};
struct RebitEditor::Choice final : public juce::Component
{
    juce::Label title;
    juce::ComboBox box;
    std::unique_ptr<juce::AudioProcessorValueTreeState::ComboBoxAttachment> attachment;
    Choice (juce::AudioProcessorValueTreeState& state, const juce::String& id,
            const juce::String& name, const juce::StringArray& options, juce::Colour accent)
    {
        title.setText (name, juce::dontSendNotification); title.setColour (juce::Label::textColourId, muted);
        title.setFont (type (10)); title.setBorderSize (juce::BorderSize<int> (0));
        box.setName (name); box.setColour (accentColourId, accent); box.addItemList (options, 1);
        addAndMakeVisible (title); addAndMakeVisible (box);
        attachment = std::make_unique<juce::AudioProcessorValueTreeState::ComboBoxAttachment> (state, id, box);
    }
    void resized() override
    {
        auto r = getLocalBounds(); title.setBounds (r.removeFromTop (16)); box.setBounds (r);
    }
};

RebitEditor::RebitEditor (RebitProcessor& p)
    : AudioProcessorEditor (p), instrument (p), theme (std::make_unique<RebitLookAndFeel>()),
      keyboard (std::make_unique<Keyboard> (p.keyboard))
{
    setLookAndFeel (theme.get()); setOpaque (true);
    auto knob = [this] (const char* id, const char* name, const char* suffix, juce::Colour accent)
    {
        auto control = std::make_unique<Knob> (instrument.state, id, name, suffix, accent);
        addAndMakeVisible (*control);
        control->slider.setColour (juce::Slider::textBoxOutlineColourId, juce::Colours::transparentBlack);
        control->slider.lookAndFeelChanged();
        knobs.push_back (std::move (control));
    };
    knob ("gain", "LEVEL", "", mint); knob ("maxPoly", "VOICES", "", mint);
    knob ("attack", "ATTACK", " s", amber); knob ("decay", "DECAY", " s", amber);
    knob ("suslevel", "SUSTAIN", "", amber); knob ("release", "RELEASE", " s", amber);
    knob ("portamentoTime", "GLIDE", " s", cyan); knob ("bendRange", "BEND RANGE", " st", cyan);
    knob ("arpeggioIntervalSliderValue", "ARP INTERVAL", " s", cyan);
    knob ("vibratoRate", "VIB PERIOD", " s", violet); knob ("vibratoDepth", "VIB DEPTH", " st", violet);
    knob ("vibratoDelay", "VIB DELAY", " s", violet); knob ("sweepInitialPitch", "SWEEP START", " st", violet);
    knob ("sweepTime", "SWEEP TIME", " s", violet);
    knob ("arpGate", "GATE", " %", cyan);knob ("arpSwing", "SWING", " %", cyan);
    knob ("arpSeconds", "STEP TIME", " s", cyan);
    auto choice = [this] (const char* id, const char* name, juce::StringArray options, juce::Colour accent)
    {
        auto control = std::make_unique<Choice> (instrument.state, id, name, options, accent);
        addAndMakeVisible (*control); choices.push_back (std::move (control));
    };
    choice ("osc", "WAVEFORM", { "Pulse / square", "Stepped triangle", "Noise" }, mint);
    choice ("duty", "PULSE WIDTH", { "12.5%", "25%", "50%" }, mint);
    choice ("noiseAlgorithm_raw", "NOISE TYPE", { "4-bit random", "1-bit long", "1-bit short" }, mint);
    choice ("monophonicBehavior_raw", "MONO BEHAVIOR", { "Legato", "Legacy arp up", "Legacy arp down", "Retrigger" }, cyan);
    choice ("arpeggioIntervalType_raw", "ARP CLOCK", { "1 frame", "2 frames", "3 frames", "96th", "64th", "48th", "32nd", "24th", "Seconds" }, cyan);
    choice ("pitchSequenceMode_raw", "PITCH UNITS", { "Semitones", "1/8 semitone" }, cyan);
    choice ("arpPattern", "PATTERN", {"Up","Down","Up / Down","Random","Played order"}, cyan);
    choice ("arpRate", "RATE", {"1/4","1/8","1/16","1/32","1/64","1/4 dotted","1/8 dotted","1/16 dotted","1/32 dotted","1/64 dotted","1/4 triplet","1/8 triplet","1/16 triplet","1/32 triplet","1/64 triplet","Seconds"}, cyan);
    choice ("arpOctaves", "OCTAVES", {"1","2","3","4"}, cyan);
    choice ("arpArticulation", "ENVELOPE", {"Retrigger","Legato"}, cyan);
    choice ("sequenceClock", "FRAME CLOCK", {"60 Hz","1/4","1/8","1/16","1/32","1/64","1/8 triplet","1/16 triplet"}, cyan);
    hostSync.setColour(accentColourId,cyan);addAndMakeVisible(hostSync);
    toggles.push_back(std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment>(instrument.state,"hostSync",hostSync));
    hostSync.setTooltip("Align musical arp and sequence clocks with host play/stop and PPQ. Seconds stays free. Without complete host timing, clocks free-run. Turn off for stopped-host audition.");
    choices[sequenceClock]->box.setTooltip("60 Hz keeps note-relative chip frames. Musical frames align to host PPQ with Host sync on; while stopped or without host timing they advance at the selected tempo rate.");
    for(auto* button:{&arpTab,&monoTab,&arpStop}){
        button->getProperties().set("sequenceSmall",true);button->setColour(accentColourId,cyan);addAndMakeVisible(*button);
    }
    arpTab.onClick=[this]{arpPanel=true;updatePerformance();};monoTab.onClick=[this]{arpPanel=false;updatePerformance();};
    arpStop.onClick=[this]{instrument.stopArpeggiator();};
    arpStop.setTooltip("Stop all voices and clear held/latched notes. Play a new chord to restart.");
    arpEnabled.setColour(accentColourId,cyan);arpLatch.setColour(accentColourId,cyan);
    addAndMakeVisible(arpEnabled);addAndMakeVisible(arpLatch);
    toggles.push_back(std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment>(instrument.state,"arpEnabled",arpEnabled));
    toggles.push_back(std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment>(instrument.state,"arpLatch",arpLatch));
    arpEnabled.setTooltip("Turn held chords into an arpeggio at any voice count. The new arp overrides legacy mono arpeggios.");
    arpLatch.setTooltip("Retain released notes. A new chord after all keys lift replaces the latched chord.");
    choices[arpRate]->box.setTooltip("Musical rates follow host tempo. Dotted steps are 1.5x; triplets are 2/3. Seconds uses Step Time.");
    choices[arpArticulation]->box.setTooltip("Retrigger restarts the envelope and sequences every step. Legato continues them at 100% Gate; shorter gates create separate notes.");
    choices[monoBehavior]->box.setTooltip("Mono modes require 1 voice and Arp off. Legacy arps are retained for existing patches.");
    ignoreWheel.setColour (accentColourId, violet); addAndMakeVisible (ignoreWheel);
    ignoreWheel.setTooltip ("When enabled, vibrato depth does not depend on MIDI CC1.");
    toggles.push_back (std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment> (instrument.state, "vibratoIgnoresWheel_raw", ignoreWheel));
    const char* names[] = { "Volume", "Pitch", "Duty" };
    const char* ids[] = { "isVolumeSequenceEnabled_raw", "isPitchSequenceEnabled_raw", "isDutySequenceEnabled_raw" };
    const char* placeholders[] = { "15,12,9,6 [4,3] | 2,1,0", "0,4,7 [12,7,4]", "0,1,2 [1,0]" };
    const juce::Colour accents[] = { mint, cyan, violet };
    for (size_t i = 0; i < 3; ++i)
    {
        sequenceEnabled[i].setButtonText (names[i]); sequenceEnabled[i].setColour (accentColourId, accents[i]);
        addAndMakeVisible (sequenceEnabled[i]);
        toggles.push_back (std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment> (instrument.state, ids[i], sequenceEnabled[i]));
        auto& editor = sequenceEditors[i]; editor.setName (juce::String (names[i]) + " sequence");
        editor.setMultiLine (false); editor.setReturnKeyStartsNewLine (false); editor.setFont (type (12, true));
        editor.setTextToShowWhenEmpty (placeholders[i], muted.withAlpha (0.5f));
        editor.setText (instrument.sequenceText ((int) i), false); lastCommittedSequences[i] = editor.getText();
        editor.setIndents (12, 8);
        editor.setTooltip (juce::String (names[i]) + " sequence: comma-separated values, [loop], | release. Press Return to apply.");
        editor.onReturnKey = [this, i] { applySequence ((int) i); };
        editor.onFocusLost = [this, i] { applySequence ((int) i); }; addAndMakeVisible (editor);
    }
    sequenceCanvas=std::make_unique<SequenceCanvas>(instrument);
    sequenceCanvas->onLaneChanged=[this](int lane){
        for(size_t i=0;i<3;++i)sequenceEditors[i].setVisible((int)i==lane);
    };
    sequenceCanvas->onCommitted=[this](int lane){
        const auto text=instrument.sequenceText(lane);
        sequenceEditors[(size_t)lane].setText(text,false);lastCommittedSequences[(size_t)lane]=text;
        sequenceEditors[(size_t)lane].setColour(juce::TextEditor::outlineColourId,line);
        sequenceStatus.setColour(juce::Label::textColourId,muted);
        sequenceStatus.setText("Draw steps. Drag LOOP / REL markers. Arrow keys fine-tune values.",juce::dontSendNotification);
    };
    addAndMakeVisible(*sequenceCanvas);
    for(size_t i=1;i<3;++i)sequenceEditors[i].setVisible(false);
    applySequences.setButtonText ("APPLY TEXT  +"); addAndMakeVisible (applySequences);
    applySequences.onClick = [this] { applySequence(sequenceCanvas->lane()); };
    sequenceStatus.setText ("Draw steps. Drag LOOP / REL markers. Arrow keys fine-tune values.", juce::dontSendNotification);
    sequenceStatus.setFont (type (10.5f, true)); sequenceStatus.setColour (juce::Label::textColourId, muted);
    addAndMakeVisible (sequenceStatus);
    keyboard->setAvailableRange (24, 108); keyboard->setKeyWidth (36); keyboard->setLowestVisibleKey (48);
    keyboard->setBlackNoteLengthProportion (0.62f); keyboard->setScrollButtonWidth (20);
    keyboard->setVelocity (0.8f, true); keyboard->setColour (juce::MidiKeyboardComponent::whiteNoteColourId, panel);
    keyboard->setColour (juce::MidiKeyboardComponent::blackNoteColourId, inset);
    keyboard->setColour (juce::MidiKeyboardComponent::shadowColourId, juce::Colours::transparentBlack);
    addAndMakeVisible (*keyboard);
    if(choices[monoBehavior]->box.getSelectedItemIndex()==1||choices[monoBehavior]->box.getSelectedItemIndex()==2)
        arpPanel=arpEnabled.getToggleState();
    setSize (1120, 930); updatePerformance();startTimerHz (15); timerCallback();
}

RebitEditor::~RebitEditor()
{
    stopTimer(); setLookAndFeel (nullptr);
}

void RebitEditor::applySequence (int index)
{
    const auto error = instrument.setSequenceText (index, sequenceEditors[(size_t) index].getText());
    const char* names[] = { "Volume", "Pitch", "Duty" };
    const auto success = error.isEmpty();
    if (success) lastCommittedSequences[(size_t) index] = sequenceEditors[(size_t) index].getText();
    sequenceEditors[(size_t) index].setColour (juce::TextEditor::outlineColourId, success ? line : amber);
    sequenceStatus.setColour (juce::Label::textColourId, success ? mint : amber);
    sequenceStatus.setText (success ? juce::String (names[index]) + " sequence applied."
                                   : juce::String (names[index]) + ": " + error,
                            juce::dontSendNotification);
}

void RebitEditor::applyAllSequences()
{
    for (int i = 0; i < 3; ++i)
    {
        const auto error = instrument.setSequenceText (i, sequenceEditors[(size_t) i].getText());
        sequenceEditors[(size_t) i].setColour (juce::TextEditor::outlineColourId, error.isEmpty() ? line : amber);
        if (error.isEmpty()) lastCommittedSequences[(size_t) i] = sequenceEditors[(size_t) i].getText();
        if (error.isNotEmpty())
        {
            sequenceStatus.setColour (juce::Label::textColourId, amber);
            const char* names[] = { "Volume", "Pitch", "Duty" };
            sequenceStatus.setText (juce::String (names[i]) + ": " + error, juce::dontSendNotification);
            return;
        }
    }
    sequenceStatus.setColour (juce::Label::textColourId, mint);
    sequenceStatus.setText ("All three sequences applied.", juce::dontSendNotification);
}

void RebitEditor::updatePerformance()
{
    arpTab.setToggleState(arpPanel,juce::dontSendNotification);monoTab.setToggleState(!arpPanel,juce::dontSendNotification);
    for(auto index:{monoBehavior,arpInterval})choices[(size_t)index]->setVisible(!arpPanel);
    for(auto index:{portamento,bend,arpSpeed})knobs[(size_t)index]->setVisible(!arpPanel);
    for(auto index:{arpPattern,arpRate,arpOctaves,arpArticulation})choices[(size_t)index]->setVisible(arpPanel);
    for(auto index:{arpGate,arpSwing,arpSeconds})knobs[(size_t)index]->setVisible(arpPanel);
    arpLatch.setVisible(arpPanel);arpStop.setVisible(arpPanel);hostSync.setVisible(arpPanel);repaint(24,346,478,184);
}

void RebitEditor::timerCallback()
{
    for (size_t i = 0; i < sequenceEditors.size(); ++i)
    {
        const auto stored = instrument.sequenceText ((int) i);
        if (stored != lastCommittedSequences[i] && !sequenceEditors[i].hasKeyboardFocus (true))
        {
            sequenceEditors[i].setText (stored, false); lastCommittedSequences[i] = stored;
            sequenceEditors[i].setColour (juce::TextEditor::outlineColourId, line);
        }
    }
    const auto oscType = choices[oscillator]->box.getSelectedItemIndex();
    choices[pulseDuty]->setEnabled (oscType == 0); choices[noiseAlgorithm]->setEnabled (oscType == 2);
    monoMode = knobs[poly]->slider.getValue() < 1.5 && !arpEnabled.getToggleState();
    choices[monoBehavior]->setEnabled (monoMode); choices[arpInterval]->setEnabled (monoMode);
    knobs[portamento]->setEnabled (monoMode && choices[monoBehavior]->box.getSelectedItemIndex() == 0);
    knobs[arpSpeed]->setEnabled (monoMode && choices[arpInterval]->box.getSelectedItemIndex() == 8);
    knobs[arpSeconds]->setEnabled(choices[arpRate]->box.getSelectedItemIndex()==15);
    midiActive = false;
    for (int note = 0; note < 128; ++note)
        if (instrument.keyboard.isNoteOnForChannels (0xffff, note)) { midiActive = true; break; }
    repaint (380, 24, 716, 112); repaint (950, 196, 130, 126);
}

void RebitEditor::paint (juce::Graphics& g)
{
    g.fillAll (background);
    // A quiet grid along the chassis, outside the controls.
    g.setColour (line.withAlpha (0.25f));
    for (int x = 16; x < getWidth(); x += 16)
        for (int y = 16; y < getHeight(); y += 16) g.fillRect (x, y, 1, 1);
    pixelSpark (g, 33, 49, mint);
    pixelLogo (g, 80, 42, 6);
    caption (g, "SMALL WAVES. BIG WORLDS.", { 33, 95, 310, 18 }, muted, 10.5f, true);
    g.setColour (mint.withAlpha (0.12f)); g.fillRoundedRectangle (33, 119, 48, 17, 3);
    caption (g, "8-BIT", { 41, 119, 40, 17 }, mint, 9, true);
    caption (g, "POLYPHONIC SYNTHESIZER", { 92, 119, 240, 17 }, muted, 9, true);

    const juce::Rectangle<float> screen (380, 24, 716, 112);
    g.setColour (juce::Colours::black.withAlpha (0.35f)); g.fillRoundedRectangle (screen.translated (0, 3), 9);
    g.setGradientFill (juce::ColourGradient (juce::Colour (0xff101e24), screen.getTopLeft(), inset, screen.getBottomRight(), false));
    g.fillRoundedRectangle (screen, 8);
    g.setColour (line.brighter (0.07f)); g.drawRoundedRectangle (screen.reduced (0.5f), 8, 1);
    caption (g, "WAVE PREVIEW", { 400, 37, 150, 17 }, muted, 9, true);
    const int oscType = choices[oscillator]->box.getSelectedItemIndex();
    const char* waveNames[] = { "PULSE", "TRIANGLE", "NOISE" };
    caption (g, waveNames[juce::jlimit (0, 2, oscType)], { 400, 59, 144, 25 }, ink, 18, true);
    g.setColour (mint.withAlpha (0.15f)); g.fillRect (401, 102, 8, 8);
    g.setColour (mint.withAlpha (midiActive ? 1.0f : 0.6f)); g.fillRect (403, 104, 4, 4);
    caption (g, midiActive ? "MIDI ACTIVE" : "READY TO PLAY", { 418, 98, 128, 17 }, midiActive ? mint : muted, 9, true);
    {
        juce::Graphics::ScopedSaveState save (g);
        g.reduceClipRegion (juce::Rectangle<int> (550, 35, 530, 90));
        for (int x = 552; x < 1080; x += 24)
        {
            g.setColour (cyan.withAlpha (0.055f)); g.drawVerticalLine (x, 36, 124);
        }
        for (int y = 40; y < 124; y += 20)
        {
            g.setColour (cyan.withAlpha (0.055f)); g.drawHorizontalLine (y, 550, 1080);
        }
        juce::Path waveform;
        for (int i = 0; i <= 384; ++i)
        {
            const float phase = (float) i / 384 * 3;
            float v = 0;
            if (oscType == 0)
            {
                constexpr float widths[] = { 0.125f, 0.25f, 0.5f };
                v = std::fmod (phase, 1.0f) < widths[juce::jlimit (0, 2, choices[pulseDuty]->box.getSelectedItemIndex())] ? 1.0f : -1.0f;
            }
            else if (oscType == 1)
                v = std::round ((1 - 4 * std::abs (std::fmod (phase, 1.0f) - 0.5f)) * 7.5f) / 7.5f;
            else
            {
                const int step = i / 4;
                v = (float) ((step * 37 + step * step * 11) % 17 - 8) / 8;
            }
            const float x = 557 + (float) i / 384 * 516, y = 80 - v * 24;
            if (i == 0) waveform.startNewSubPath (x, y);
            else { waveform.lineTo (x, waveform.getCurrentPosition().y); waveform.lineTo (x, y); }
        }
        bloom (g, waveform, cyan.withAlpha (0.9f), 1.4f);
        for (int y = 36; y < 124; y += 3)
        {
            g.setColour (juce::Colours::black.withAlpha (0.1f)); g.drawHorizontalLine (y, 550, 1080);
        }
    }

    const char* clockLabels[]={"FREE CLOCK","HOST STOP","HOST PLAY","FREE / NO HOST"};
    const int clock=juce::jlimit(0,3,instrument.clockStatus());
    g.setColour(inset.withAlpha(.9f));g.fillRoundedRectangle(943,109,134,15,3);
    caption(g,clockLabels[clock],{950,109,127,15},clock==2?cyan:muted,8.5f,true);

    section (g, { 24, 160, 420, 170 }, "01", "OSCILLATOR", mint);
    section (g, { 460, 160, 636, 170 }, "02", "AMPLITUDE", amber);
    // A relative-time ADSR diagram follows the current amplitude parameters.
    const float a = 0.12f + std::sqrt ((float) knobs[attack]->slider.getValue()) * 0.18f;
    const float d = 0.12f + std::sqrt ((float) knobs[decay]->slider.getValue()) * 0.18f;
    const float r = 0.12f + std::sqrt ((float) knobs[release]->slider.getValue()) * 0.18f;
    const float total = a + d + r + 0.35f;
    const float x0 = 957, y0 = 276, width = 115, height = 54;
    const float ax = x0 + width * a / total, dx = ax + width * d / total;
    const float rx = x0 + width * (a + d + 0.35f) / total;
    const float sy = y0 - (float) knobs[sustain]->slider.getValue() * height;
    g.setColour (line); g.drawHorizontalLine (277, x0, x0 + width);
    juce::Path envelope; envelope.startNewSubPath (x0, y0); envelope.lineTo (ax, y0 - height);
    envelope.lineTo (dx, sy); envelope.lineTo (rx, sy); envelope.lineTo (x0 + width, y0);
    auto fill = envelope; fill.closeSubPath();
    g.setGradientFill (juce::ColourGradient (amber.withAlpha (0.12f), x0, y0 - height, amber.withAlpha (0.0f), x0, y0, false));
    g.fillPath (fill); bloom (g, envelope, amber.withAlpha (0.8f), 1.2f);
    caption (g, "A / D / S / R", { 958, 291, 125, 18 }, muted, 8.5f, true);

    section (g, { 24, 346, 478, 184 }, "03", "PERFORMANCE", cyan);

    section (g, { 518, 346, 578, 184 }, "04", "MODULATION", violet);
    section (g, { 24, 546, 1072, 262 }, "05", "FRAME SEQUENCES", cyan);
    caption (g, choices[sequenceClock]->box.getSelectedItemIndex()==0?"60 Hz":"MUSICAL", { 1015, 555, 62, 20 }, cyan, 10, true);
    caption (g, "TEXT", { 40, 743, 56, 31 }, muted, 10, true);
    caption (g, "PLAY", { 25, 818, 60, 18 }, mint, 10, true);
    caption (g, "MIDI input or click the keys", { 81, 818, 280, 18 }, muted, 10);
    caption (g, "SCROLL TO CHANGE OCTAVE", { 882, 818, 214, 18 }, muted, 9, true);
    g.setColour (line); g.drawHorizontalLine (907, 24, 1096);
    caption (g, "REBIT RUST", { 24, 911, 140, 15 }, muted, 8.5f, true);
    caption (g, "PULSE / TRIANGLE / NOISE", { 426, 911, 300, 15 }, muted, 8.5f, true);
    caption (g, juce::String ("v") + JucePlugin_VersionString, { 1048, 911, 48, 15 }, muted, 8.5f, true);
}

void RebitEditor::resized()
{
    choices[oscillator]->setBounds (40, 202, 198, 48);
    choices[pulseDuty]->setBounds (40, 267, 94, 46);
    choices[noiseAlgorithm]->setBounds (145, 267, 94, 46);
    knobs[gain]->setBounds (249, 202, 88, 113); knobs[poly]->setBounds (341, 202, 88, 113);
    for (int i = 0; i < 4; ++i) knobs[(size_t) attack + (size_t) i]->setBounds (475 + i * 117, 202, 108, 113);
    choices[monoBehavior]->setBounds (40, 385, 214, 45);
    choices[arpInterval]->setBounds (270, 385, 216, 45);
    knobs[portamento]->setBounds (56, 439, 110, 83);
    knobs[bend]->setBounds (201, 439, 110, 83);
    knobs[arpSpeed]->setBounds (353, 439, 112, 83);
    for (int i = 0; i < 5; ++i) knobs[(size_t) vibratoRate + (size_t) i]->setBounds (532 + i * 111, 387, 108, 109);
    ignoreWheel.setBounds (540, 502, 340, 22);
    arpTab.setBounds(252,355,52,24);monoTab.setBounds(310,355,58,24);arpEnabled.setBounds(390,355,98,24);
    choices[arpPattern]->setBounds(40,385,130,45);choices[arpRate]->setBounds(180,385,116,45);
    choices[arpOctaves]->setBounds(306,385,62,45);choices[arpArticulation]->setBounds(378,385,108,45);
    knobs[arpGate]->setBounds(44,439,100,83);knobs[arpSwing]->setBounds(154,439,100,83);knobs[arpSeconds]->setBounds(264,439,100,83);
    arpLatch.setBounds(382,438,104,22);hostSync.setBounds(382,463,104,22);arpStop.setBounds(382,491,104,24);
    for (int i = 0; i < 3; ++i)
    {
        sequenceEnabled[(size_t) i].setBounds (680 + i * 105, 554, 104, 24);
        sequenceEditors[(size_t) i].setBounds (100, 744, 736, 30);
    }
    choices[sequenceClock]->setBounds(250,548,155,30);
    choices[pitchMode]->setBounds (420, 548, 195, 30);
    applySequences.setBounds (868, 744, 208, 30);
    sequenceStatus.setBounds (96, 781, 980, 20);
    sequenceCanvas->setBounds(40,584,1032,148);
    keyboard->setBounds (24, 843, 1072, 54);
}
