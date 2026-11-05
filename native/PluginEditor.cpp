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
        slider.setNumDecimalPlacesToDisplay (id == "maxPoly" || id == "bendRange" || id == "sweepInitialPitch" || id == "arpGate" || id == "arpSwing" || id.endsWith("Transpose") || id.endsWith("Detune") || id=="fxBits" || id=="fxRate" || id=="fxMix" ? 0 : id == "gain" || id == "suslevel" || id.endsWith("Gain") || id.endsWith("Pan") ? 2 : 3);
        if(id=="fxDrive")slider.setNumDecimalPlacesToDisplay(1);
        slider.textFromValueFunction = nullptr;
        if(id.endsWith("Pan"))slider.textFromValueFunction=[](double value){return juce::String(std::abs(value)<1e-6?0.:value,2);};
        slider.updateText();
    }
    void bind(juce::AudioProcessorValueTreeState& state,const char* id) {
        attachment.reset();
        if(auto* p=state.getParameter(id))slider.setDoubleClickReturnValue(true,p->convertFrom0to1(p->getDefaultValue()));
        attachment=std::make_unique<juce::AudioProcessorValueTreeState::SliderAttachment>(state,id,slider);
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
    void bind(juce::AudioProcessorValueTreeState& state,const char* id) {
        attachment.reset();attachment=std::make_unique<juce::AudioProcessorValueTreeState::ComboBoxAttachment>(state,id,box);
    }
    void resized() override
    {
        auto r = getLocalBounds(); title.setBounds (r.removeFromTop (16)); box.setBounds (r);
    }
};

struct RebitEditor::RouteRow final : public juce::Component
{
    juce::ComboBox source, destination;
    juce::Slider amount;
    std::unique_ptr<juce::AudioProcessorValueTreeState::ComboBoxAttachment> sourceAttachment,
        destinationAttachment;
    std::unique_ptr<juce::AudioProcessorValueTreeState::SliderAttachment> amountAttachment;
    int number = 0;
    RouteRow (juce::AudioProcessorValueTreeState& state, int index) : number (index)
    {
        const juce::String prefix = "Mod " + juce::String (index + 1);
        const juce::StringArray sources { "Off", "Velocity", "Mod wheel", "Aftertouch",
                                          "Timbre CC74", "LFO 1", "LFO 2", "Macro 1",
                                          "Macro 2", "Macro 3", "Macro 4", "LFO 1 unipolar",
                                          "LFO 2 unipolar" };
        const juce::StringArray destinations { "Off", "Pitch", "Pulse width", "Level",
                                               "Layer balance", "FX drive", "FX mix" };
        source.setName (prefix + " source"); destination.setName (prefix + " destination");
        source.addItemList (sources, 1); destination.addItemList (destinations, 1);
        source.setColour (accentColourId, violet); destination.setColour (accentColourId, violet);
        source.setTooltip ("Modulation source for route " + juce::String (index + 1) + ". Velocity and aftertouch are per note; LFOs, macros and wheels are shared.");
        destination.setTooltip ("Modulation destination for route " + juce::String (index + 1) + ". Pulse width and pitch are per note; layer balance and effects are shared.");
        amount.setName (prefix + " amount");
        amount.setSliderStyle (juce::Slider::LinearHorizontal);
        amount.setTextBoxStyle (juce::Slider::TextBoxRight, false, 50, 22);
        amount.setRange (-100., 100., 1.);
        amount.setTextValueSuffix ("%");
        amount.setNumDecimalPlacesToDisplay (0);
        amount.setDoubleClickReturnValue (true, 0.);
        amount.setColour (accentColourId, violet);
        amount.setTooltip ("Bipolar amount for route " + juce::String (index + 1) + ". Negative inverts the source. Pitch spans +/-12 semitones, pulse width +/-50%, and level up to 2x.");
        addAndMakeVisible (source); addAndMakeVisible (destination); addAndMakeVisible (amount);
        sourceAttachment = std::make_unique<juce::AudioProcessorValueTreeState::ComboBoxAttachment> (
            state, "mod" + juce::String (index + 1) + "Source", source);
        destinationAttachment = std::make_unique<juce::AudioProcessorValueTreeState::ComboBoxAttachment> (
            state, "mod" + juce::String (index + 1) + "Dest", destination);
        amountAttachment = std::make_unique<juce::AudioProcessorValueTreeState::SliderAttachment> (
            state, "mod" + juce::String (index + 1) + "Amount", amount);
    }
    void paint (juce::Graphics& g) override
    {
        caption (g, juce::String (number + 1), { 0, 0, 14, getHeight() }, muted, 10, true);
    }
    void resized() override
    {
        auto r = getLocalBounds();
        r.removeFromLeft (16);
        source.setBounds (r.removeFromLeft (128).reduced (0, 3));
        r.removeFromLeft (6);
        destination.setBounds (r.removeFromLeft (140).reduced (0, 3));
        r.removeFromLeft (6);
        amount.setBounds (r.reduced (0, 3));
    }
};

// The display polls only the processor's lock-free published copy. It never
// touches Rust engine state. The waveform behind it remains a parameter preview.
struct RebitEditor::PlaybackDisplay final : juce::Component, juce::SettableTooltipClient, private juce::Timer
{
    RebitProcessor& processor;
    juce::TextButton clip { "CLIP" };
    PlaybackSnapshot playback;
    uint32_t revision=0;
    double lastAudio=0.;
    bool idle=true;
    const juce::Colour red {0xffff727e};
    explicit PlaybackDisplay(RebitProcessor& p) : processor(p) {
        setName("Output and voice activity");
        setTooltip("Actual output sample peaks, in dBFS. Mint tiles are held/pedalled voices; amber tiles are releases. +FADE counts brief voice-removal continuations. The waveform is a parameter preview.");
        clip.setName("Clear output clipping");clip.setTooltip("Latched when output reaches 0 dBFS. Click to clear; further clipping lights it again. Lower LEVEL or reduce stacked voices. Output is not limited.");
        clip.getProperties().set("sequenceSmall",true);clip.setColour(accentColourId,red);
        addAndMakeVisible(clip);clip.onClick=[this]{processor.clearClip();refresh();};
        refresh();startTimerHz(30);
    }
    ~PlaybackDisplay() override {stopTimer();}
    static float db(float value) {return value>0.f?20.f*std::log10(value):-120.f;}
    void refresh() {
        playback=processor.playbackInfo();
        const auto now=juce::Time::getMillisecondCounterHiRes();
        if(playback.revision!=revision){revision=playback.revision;lastAudio=now;}
        // A suspended host sends no silence buffers. Do not leave a frozen meter
        // or frozen activity on screen; clip history remains until acknowledged.
        idle=revision==0 || now-lastAudio>(double)playback.idleAfterMs;
        if(idle){playback.peak=playback.heldPeak=0.f;playback.active=playback.held=playback.released=playback.fading=0;playback.layers={};}
        clip.setToggleState(playback.clipped,juce::dontSendNotification);
        repaint();
    }
    void timerCallback() override {refresh();}
    void resized() override {clip.setBounds(374,68,61,20);}
    void paint(juce::Graphics& g) override {
        g.setColour(line.withAlpha(.75f));g.drawVerticalLine(361,3.f,88.f);
        caption(g,"OUTPUT",{374,0,80,14},muted,8.5f,true);
        const auto peakDb=db(playback.heldPeak);
        const auto peakText=playback.heldPeak>0.f?juce::String(peakDb,1)+" dBFS":juce::String("-INF dBFS");
        caption(g,peakText,{374,17,148,18},playback.clipped?red:ink,12.f,true);
        const float levelDb=db(playback.peak);
        constexpr int segments=22;
        constexpr float x=374.f,y=40.f,width=148.f;
        for(int i=0;i<segments;++i){
            const float threshold=-60.f+3.f*(float)i;
            const auto colour=threshold>=0.f?red:threshold>=-12.f?amber:mint;
            const juce::Rectangle<float> bar(x+(float)i*width/(float)segments,y,width/(float)segments-2.f,10.f);
            g.setColour(colour.withAlpha(levelDb>=threshold?0.9f:0.09f));g.fillRect(bar);
        }
        if(playback.heldPeak>0.f){
            const float marker=x+juce::jlimit(0.f,1.f,(peakDb+60.f)/66.f)*(width-1.f);
            g.setColour(ink);g.fillRect(marker,y-3.f,1.f,3.f);
        }
        caption(g,"-60",{374,52,32,12},muted,7.f,true);
        caption(g,"-24",{450,52,32,12},muted,7.f,true);
        caption(g,"0",{506,52,16,12},muted,7.f,true);
        caption(g,"0 dBFS",{444,70,78,16},playback.clipped?red:muted,8.f,true);
        const int limit=(int)processor.state.getRawParameterValue("maxPoly")->load(std::memory_order_relaxed);
        const auto activity=idle?juce::String("AUDIO IDLE"):
            juce::String::formatted("A %02u/%02d   B %02u/%02d   +%02u FADE",playback.layers[0].active,limit,playback.layers[1].active,limit,playback.fading);
        caption(g,activity,{7,60,345,14},idle?muted:ink,8.f,true);
        for(int layer=0;layer<2;++layer){
            const auto info=playback.layers[(size_t)layer];const float tileY=77.f+(float)layer*7.f;
            caption(g,layer==0?"A":"B",{7,(int)tileY-2,10,9},layer==0?mint:violet,7.f,true);
            for(int i=0;i<64;++i){
                const bool held=(uint32_t)i<info.held,released=(uint32_t)i>=info.held&&(uint32_t)i<info.active;
                const auto colour=held?(layer==0?mint:violet):released?amber:line;
                const float tileX=20.f+(float)i*5.13f;
                g.setColour(colour.withAlpha(held||released?.95f:.7f));g.fillRect(tileX,tileY,3.f,4.f);
            }
        }
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
    knob ("gain", "MASTER", "", mint); knob ("maxPoly", "VOICES", "", mint);
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
    for(auto* button:{&arpTab,&monoTab,&modTab,&arpStop}){
        button->getProperties().set("sequenceSmall",true);button->setColour(accentColourId,cyan);addAndMakeVisible(*button);
    }
    modTab.setColour(accentColourId,violet);modTab.setName("Modulation page");
    arpTab.onClick=[this]{performancePage=0;updatePerformance();};monoTab.onClick=[this]{performancePage=1;updatePerformance();};
    modTab.onClick=[this]{performancePage=2;updatePerformance();};
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
    knobs[poly]->slider.setTooltip("Keep held keys when changing Voices. Mono keeps the latest key; raising Voices restores held keys. Removed voices fade briefly.");
    choices[oscillator]->box.setTooltip("Waveform changes crossfade over 5 ms while held notes keep their phase and sequence position.");
    choices[pulseDuty]->box.setTooltip("Manual pulse-width edits crossfade over 5 ms. Duty sequence frames retain their chip-style steps.");
    choices[noiseAlgorithm]->box.setTooltip("Noise types crossfade over 5 ms during playback.");
    for(int i=0;i<4;++i){routeRows[(size_t)i]=std::make_unique<RouteRow>(instrument.state,i);addAndMakeVisible(*routeRows[(size_t)i]);}
    const juce::StringArray lfoShapes{"Sine","Triangle","Square","Ramp up","Ramp down","Sample & hold"};
    const char* shapeIds[]={"lfo1Shape","lfo2Shape"};const char* rateIds[]={"lfo1Rate","lfo2Rate"};const char* retriggerIds[]={"lfo1Retrigger","lfo2Retrigger"};
    for(int i=0;i<2;++i){
        lfoShape[(size_t)i]=std::make_unique<Choice>(instrument.state,shapeIds[i],i==0?"LFO 1 SHAPE":"LFO 2 SHAPE",lfoShapes,violet);
        lfoShape[(size_t)i]->box.setTooltip("Shared modulation LFO. Sample & hold changes once per cycle. Per-note routes use each voice's own copy.");
        addAndMakeVisible(*lfoShape[(size_t)i]);
        lfoRate[(size_t)i]=std::make_unique<Knob>(instrument.state,rateIds[i],i==0?"LFO 1 RATE":"LFO 2 RATE"," Hz",violet);
        lfoRate[(size_t)i]->slider.setTooltip("Free-running rate from 0.01 to 20 Hz. Host tempo sync is not applied to modulation LFOs.");
        addAndMakeVisible(*lfoRate[(size_t)i]);
        lfoRetrigger[(size_t)i].setButtonText(i==0?"Retrigger 1":"Retrigger 2");lfoRetrigger[(size_t)i].setName(i==0?"LFO 1 retrigger":"LFO 2 retrigger");
        lfoRetrigger[(size_t)i].setColour(accentColourId,violet);
        lfoRetrigger[(size_t)i].setTooltip("Restart this LFO at phase zero on the first note of a chord. Off keeps it free-running.");
        addAndMakeVisible(lfoRetrigger[(size_t)i]);
        toggles.push_back(std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment>(instrument.state,retriggerIds[i],lfoRetrigger[(size_t)i]));
    }
    const char* macroIds[]={"macro1","macro2","macro3","macro4"};
    for(int i=0;i<4;++i){
        macroKnobs[(size_t)i]=std::make_unique<Knob>(instrument.state,macroIds[i],"MACRO "+juce::String(i+1),"",violet);
        macroKnobs[(size_t)i]->slider.setTooltip("Performance macro, 0 to 100%. Assign it as a source in any route. Values are saved with the patch.");
        addAndMakeVisible(*macroKnobs[(size_t)i]);
    }
    mpeOn.setColour(accentColourId,violet);mpeOn.setName("MPE enabled");mpeOn.setTooltip("MPE lower-zone mode. Per-channel pitch bend, channel pressure and poly aftertouch work per note; bend range follows MPE BEND. MIDI is forwarded raw, so no extra bridge is required.");
    mpeBend=std::make_unique<Knob>(instrument.state,"mpeBendRange","MPE BEND"," st",violet);
    mpeBend->slider.setTooltip("Pitch-bend range used when MPE is enabled. Typical controllers use 48 semitones.");
    addAndMakeVisible(mpeOn);addAndMakeVisible(*mpeBend);
    toggles.push_back(std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment>(instrument.state,"mpeEnabled",mpeOn));
    for(auto* button:{&vibTab,&lfoTab,&macroTab}){button->getProperties().set("sequenceSmall",true);button->setColour(accentColourId,violet);addAndMakeVisible(*button);}
    vibTab.setName("Vibrato page");lfoTab.setName("LFO page");macroTab.setName("Macros page");
    vibTab.onClick=[this]{modulationPage=0;updateModulationPanel();};lfoTab.onClick=[this]{modulationPage=1;updateModulationPanel();};macroTab.onClick=[this]{modulationPage=2;updateModulationPanel();};
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
        laneAttachments[i]=std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment>(instrument.state,ids[i],sequenceEnabled[i]);
        auto& editor = sequenceEditors[i]; editor.setName (juce::String (names[i]) + " sequence");
        editor.setMultiLine (false); editor.setReturnKeyStartsNewLine (false); editor.setFont (type (12, true));
        editor.setTextToShowWhenEmpty (placeholders[i], muted.withAlpha (0.5f));
        editor.setText (instrument.sequenceText ((int) i,selectedLayer), false); lastCommittedSequences[i] = editor.getText();
        editor.setIndents (12, 8);
        editor.setTooltip (juce::String (names[i]) + " sequence: comma-separated values, [loop], | release. Press Return to apply.");
        editor.onReturnKey = [this, i] { applySequence ((int) i); };
        editor.onFocusLost = [this, i] { if(!switchingLayer)applySequence ((int) i); }; addAndMakeVisible (editor);
    }
    sequenceCanvas=std::make_unique<SequenceCanvas>(instrument);
    sequenceCanvas->onLaneChanged=[this](int lane){
        for(size_t i=0;i<3;++i)sequenceEditors[i].setVisible((int)i==lane);
    };
    sequenceCanvas->onCommitted=[this](int lane){
        const auto text=instrument.sequenceText(lane,selectedLayer);
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
        performancePage=arpEnabled.getToggleState()?0:1;
    playbackDisplay=std::make_unique<PlaybackDisplay>(instrument);addAndMakeVisible(*playbackDisplay);
    for(auto* button:{&editA,&editB}){button->getProperties().set("sequenceSmall",true);addAndMakeVisible(*button);}
    editA.setName("Edit layer A");editB.setName("Edit layer B");
    editA.onClick=[this]{selectLayer(0);};editB.onClick=[this]{selectLayer(1);};
    editA.setColour(accentColourId,mint);editB.setColour(accentColourId,violet);editA.setToggleState(true,juce::dontSendNotification);
    layerAOn.setColour(accentColourId,mint);layerBOn.setColour(accentColourId,violet);addAndMakeVisible(layerAOn);addAndMakeVisible(layerBOn);
    layerAOn.setTooltip("Layer A audio on/off. MIDI and the shared performance clock continue while muted.");
    layerBOn.setTooltip("Layer B audio on/off. Enabling starts its own envelope/sequences from the held chord and current arp step. Disabled B skips synthesis after the short fade.");
    toggles.push_back(std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment>(instrument.state,"layerAEnabled",layerAOn));
    toggles.push_back(std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment>(instrument.state,"layerBEnabled",layerBOn));
    const char* mixIds[]={"layerAGain","layerAPan","layerATranspose","layerADetune"};
    const char* mixNames[]={"LAYER LEVEL","PAN","TRANSPOSE","DETUNE"};const char* suffixes[]={"",""," st"," ct"};
    for(size_t i=0;i<4;++i){
        layerKnobs[i]=std::make_unique<Knob>(instrument.state,mixIds[i],mixNames[i],suffixes[i],mint);addAndMakeVisible(*layerKnobs[i]);
        layerKnobs[i]->slider.setColour(juce::Slider::textBoxOutlineColourId,juce::Colours::transparentBlack);layerKnobs[i]->slider.lookAndFeelChanged();
    }
    layerKnobs[1]->slider.setTooltip("Stereo balance. Centre preserves unity in both channels; hard left/right silences the opposite channel. Mono outputs average left/right.");
    soundMode=std::make_unique<Choice>(instrument.state,"layerASoundMode","SOUND MODE",juce::StringArray{"Raw","Clean"},mint);
    soundMode->box.setName("Layer sound mode");soundMode->box.setTooltip("Raw preserves the original pulse and stepped triangle. Clean reduces oscillator aliasing with smoothed edges and a continuous triangle. Noise stays unchanged. Switching fades over 5 ms.");addAndMakeVisible(*soundMode);
    effectsOn.setName("Effects enabled");effectsOn.setColour(accentColourId,amber);effectsOn.setTooltip("Enable shared output effects. Off bypasses all effects after a short fade. Neutral settings: Drive 0 dB, Bits 24, Rate 1x. Metering follows the processed output.");addAndMakeVisible(effectsOn);
    toggles.push_back(std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment>(instrument.state,"fxEnabled",effectsOn));
    const char* fxIds[]={"fxDrive","fxBits","fxRate","fxMix"};const char* fxNames[]={"DRIVE","BITS","RATE DIV","MIX"};const char* fxSuffixes[]={" dB"," bit"," x"," %"};
    for(size_t i=0;i<4;++i){
        effectKnobs[i]=std::make_unique<Knob>(instrument.state,fxIds[i],fxNames[i],fxSuffixes[i],amber);addAndMakeVisible(*effectKnobs[i]);
        effectKnobs[i]->slider.setColour(juce::Slider::textBoxOutlineColourId,juce::Colours::transparentBlack);effectKnobs[i]->slider.lookAndFeelChanged();
    }
    effectKnobs[0]->slider.setTooltip("Normalized soft saturation. 0 dB is neutral; higher drive adds harmonics. This stage can create aliasing even with Clean oscillators.");
    effectKnobs[1]->slider.setTooltip("Output quantization, 4 to 24 bits. 24 bypasses quantization. Values above full scale are not clipped by this stage.");
    effectKnobs[2]->slider.setTooltip("Stereo-linked sample-and-hold. 1x is neutral; 4x holds each captured sample for four host samples. Effective rate is host sample rate / divider.");
    effectKnobs[3]->slider.setTooltip("Blend dry and processed output. FX off always returns the dry signal after its short transition.");
    presetBox.setName("Preset browser");presetBox.setColour(accentColourId,mint);
    presetBox.setTooltip("Factory patches. Selecting a preset resets the sound and sequences, then loads the patch. Use UNDO to return to the previous patch.");
    presetPrev.setName("Previous preset");presetNext.setName("Next preset");
    presetPrev.getProperties().set("sequenceSmall",true);presetNext.getProperties().set("sequenceSmall",true);
    presetPrev.setColour(accentColourId,mint);presetNext.setColour(accentColourId,mint);
    presetStatus.setFont(type(10,true));presetStatus.setColour(juce::Label::textColourId,muted);
    presetStatus.setText("Factory presets. Select a patch to load; UNDO restores the previous patch.",juce::dontSendNotification);
    refreshPresets();
    presetBox.onChange=[this]{if(loadingPreset)return;const auto index=presetBox.getSelectedItemIndex();if(index>=0){instrument.loadPreset(index);presetStatus.setText("Loaded "+presetBox.getText(),juce::dontSendNotification);undoButton.setEnabled(true);}};
    presetPrev.onClick=[this]{if(presetBox.getNumItems()==0)return;presetBox.setSelectedItemIndex(juce::jmax(0,presetBox.getSelectedItemIndex()-1),juce::sendNotificationSync);};
    presetNext.onClick=[this]{if(presetBox.getNumItems()==0)return;presetBox.setSelectedItemIndex(juce::jmin(presetBox.getNumItems()-1,presetBox.getSelectedItemIndex()+1),juce::sendNotificationSync);};
    addAndMakeVisible(presetBox);addAndMakeVisible(presetPrev);addAndMakeVisible(presetNext);addAndMakeVisible(presetStatus);
    seedEditor.setName("Random seed");seedEditor.setText("1",false);seedEditor.setInputRestrictions(9,"0123456789");
    seedEditor.setFont(type(12,true));seedEditor.setIndents(8,5);
    seedEditor.setTooltip("Deterministic random seed. The same starting patch, seed and locks produce the same randomization.");
    newSeed.setName("New seed");newSeed.getProperties().set("sequenceSmall",true);newSeed.setColour(accentColourId,violet);
    newSeed.setTooltip("Choose a new deterministic seed without randomizing yet.");
    newSeed.onClick=[this]{seedEditor.setText(juce::String(juce::Random::getSystemRandom().nextInt(999999999)+1),false);};
    const char* lockNames[]={"OSC","ENV","SEQ","MOD","FX","MIX"};
    const char* lockTips[]={"Lock oscillator and sound mode settings.","Lock both layers' ADSR envelopes.","Lock sequence text, lane enables and frame clock.","Lock matrix routes, LFOs and macros.","Lock the shared effects chain.","Lock layers, voices, levels, pans, tuning and master gain."};
    for(int i=0;i<6;++i){randomLocks[(size_t)i].setButtonText(lockNames[i]);randomLocks[(size_t)i].setName(juce::String("Lock ")+lockNames[i]);randomLocks[(size_t)i].setColour(accentColourId,violet);randomLocks[(size_t)i].setTooltip(lockTips[i]);addAndMakeVisible(randomLocks[(size_t)i]);}
    randomStatus.setFont(type(10,true));randomStatus.setColour(juce::Label::textColourId,muted);
    randomStatus.setText("Seed 1 - all sections unlocked.",juce::dontSendNotification);
    randomizeButton.setName("Randomize");randomizeButton.setTooltip("Randomize every unlocked section using the seed. Undo restores the previous patch.");
    undoButton.setName("Undo");undoButton.setTooltip("Restore the patch from before the last preset load or randomization.");
    undoButton.setEnabled(false);
    randomizeButton.onClick=[this]{
        auto digits=seedEditor.getText().retainCharacters("0123456789");
        const auto seed=digits.isEmpty()?1u:(uint32_t)juce::jlimit(1,999999999,digits.getIntValue());
        seedEditor.setText(juce::String(seed),false);
        int locked=0;for(int i=0;i<6;++i)if(randomLocks[(size_t)i].getToggleState())locked|=1<<i;
        instrument.randomize(seed,locked);
        randomStatus.setText("Seed "+juce::String(seed)+(locked!=0?" - locked sections kept.":" - all sections randomized."),juce::dontSendNotification);
        presetStatus.setText("Patch randomized. UNDO restores the previous state.",juce::dontSendNotification);
        undoButton.setEnabled(instrument.canUndo());
    };
    undoButton.onClick=[this]{instrument.undo();randomStatus.setText("Last preset or randomization undone.",juce::dontSendNotification);presetStatus.setText("Previous patch restored.",juce::dontSendNotification);undoButton.setEnabled(instrument.canUndo());};
    addAndMakeVisible(seedEditor);addAndMakeVisible(newSeed);addAndMakeVisible(randomizeButton);addAndMakeVisible(undoButton);addAndMakeVisible(randomStatus);
    setSize (1320, 1080); updatePerformance();updateModulationPanel();startTimerHz (15); timerCallback();
}

RebitEditor::~RebitEditor()
{
    stopTimer(); setLookAndFeel (nullptr);
}

void RebitEditor::refreshPresets() {
    loadingPreset=true;
    const auto selected=presetBox.getSelectedItemIndex();
    presetBox.clear(juce::dontSendNotification);
    for(int i=0;i<instrument.presetCount();++i)presetBox.addItem(instrument.presetLabel(i),i+1);
    if(selected>=0&&selected<presetBox.getNumItems())presetBox.setSelectedItemIndex(selected,juce::dontSendNotification);
    else if(presetBox.getNumItems()>0)presetBox.setSelectedItemIndex(0,juce::dontSendNotification);
    loadingPreset=false;
}

void RebitEditor::selectLayer(int layer) {
    layer=juce::jlimit(0,1,layer);if(layer==selectedLayer)return;
    // Validate drafts before rebinding; a focus-loss callback must never write
    // the previous layer's text into the newly selected layer.
    for(size_t lane=0;lane<3;++lane)if(sequenceEditors[lane].getText()!=lastCommittedSequences[lane]){
        const auto error=instrument.setSequenceText((int)lane,sequenceEditors[lane].getText(),selectedLayer);
        if(error.isNotEmpty()){sequenceStatus.setText(error,juce::dontSendNotification);sequenceStatus.setColour(juce::Label::textColourId,amber);return;}
    }
    switchingLayer=true;sequenceCanvas->selectLayer(layer);selectedLayer=layer;
    const char* soundKnobs[]={"attack","decay","suslevel","release"};
    for(size_t i=0;i<4;++i)knobs[(size_t)attack+i]->bind(instrument.state,RebitProcessor::layerParameter(soundKnobs[i],layer));
    const char* soundChoices[]={"osc","duty","noiseAlgorithm_raw","pitchSequenceMode_raw"};
    const size_t choiceIndices[]={oscillator,pulseDuty,noiseAlgorithm,pitchMode};
    for(size_t i=0;i<4;++i)choices[choiceIndices[i]]->bind(instrument.state,RebitProcessor::layerParameter(soundChoices[i],layer));
    const char* lanes[]={"isVolumeSequenceEnabled_raw","isPitchSequenceEnabled_raw","isDutySequenceEnabled_raw"};
    for(size_t i=0;i<3;++i){
        laneAttachments[i].reset();laneAttachments[i]=std::make_unique<juce::AudioProcessorValueTreeState::ButtonAttachment>(instrument.state,RebitProcessor::layerParameter(lanes[i],layer),sequenceEnabled[i]);
        const auto text=instrument.sequenceText((int)i,layer);sequenceEditors[i].setText(text,false);lastCommittedSequences[i]=text;
    }
    const char* aIds[]={"layerAGain","layerAPan","layerATranspose","layerADetune"};
    const char* bIds[]={"layerBGain","layerBPan","layerBTranspose","layerBDetune"};
    for(size_t i=0;i<4;++i){layerKnobs[i]->bind(instrument.state,layer==0?aIds[i]:bIds[i]);layerKnobs[i]->slider.setColour(accentColourId,layer==0?mint:violet);}
    soundMode->bind(instrument.state,layer==0?"layerASoundMode":"layerBSoundMode");
    editA.setToggleState(layer==0,juce::dontSendNotification);editB.setToggleState(layer==1,juce::dontSendNotification);
    sequenceStatus.setText(layer==0?"Editing layer A. Performance and modulation are shared.":"Editing layer B. Performance and modulation are shared.",juce::dontSendNotification);
    switchingLayer=false;timerCallback();repaint();
}

void RebitEditor::applySequence (int index)
{
    const auto error = instrument.setSequenceText (index, sequenceEditors[(size_t) index].getText(),selectedLayer);
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
        const auto error = instrument.setSequenceText (i, sequenceEditors[(size_t) i].getText(),selectedLayer);
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
    arpTab.setToggleState(performancePage==0,juce::dontSendNotification);
    monoTab.setToggleState(performancePage==1,juce::dontSendNotification);
    modTab.setToggleState(performancePage==2,juce::dontSendNotification);
    const bool arp=performancePage==0,mono=performancePage==1,mod=performancePage==2;
    for(auto index:{monoBehavior,arpInterval})choices[(size_t)index]->setVisible(mono);
    for(auto index:{portamento,bend,arpSpeed})knobs[(size_t)index]->setVisible(mono);
    for(auto index:{arpPattern,arpRate,arpOctaves,arpArticulation})choices[(size_t)index]->setVisible(arp);
    for(auto index:{arpGate,arpSwing,arpSeconds})knobs[(size_t)index]->setVisible(arp);
    arpLatch.setVisible(arp);arpStop.setVisible(arp);hostSync.setVisible(arp);arpEnabled.setVisible(arp);
    for(auto& row:routeRows)if(row)row->setVisible(mod);
    repaint(24,346,478,184);
}
void RebitEditor::updateModulationPanel()
{
    vibTab.setToggleState(modulationPage==0,juce::dontSendNotification);
    lfoTab.setToggleState(modulationPage==1,juce::dontSendNotification);
    macroTab.setToggleState(modulationPage==2,juce::dontSendNotification);
    const bool vibrato=modulationPage==0,lfo=modulationPage==1,macros=modulationPage==2;
    for(auto index:{vibratoRate,vibratoDepth,vibratoDelay,sweepPitch,sweepTime})knobs[(size_t)index]->setVisible(vibrato);
    ignoreWheel.setVisible(vibrato);
    for(auto& shape:lfoShape)if(shape)shape->setVisible(lfo);
    for(auto& rate:lfoRate)if(rate)rate->setVisible(lfo);
    for(auto& retrigger:lfoRetrigger)retrigger.setVisible(lfo);
    for(auto& macro:macroKnobs)if(macro)macro->setVisible(macros);
    mpeOn.setVisible(macros);if(mpeBend)mpeBend->setVisible(macros);
    repaint(518,346,578,184);
}

void RebitEditor::timerCallback()
{
    for (size_t i = 0; i < sequenceEditors.size(); ++i)
    {
        const auto stored = instrument.sequenceText ((int) i,selectedLayer);
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
    knobs[bend]->setEnabled(!mpeOn.getToggleState());
    for(auto& knob:effectKnobs)if(knob)knob->setEnabled(effectsOn.getToggleState());
    undoButton.setEnabled(instrument.canUndo());
    repaint(1120,532,176,272);
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
    caption (g, midiActive ? "KEYS HELD" : "NO KEYS HELD", { 418, 98, 128, 17 }, midiActive ? mint : muted, 9, true);
    {
        juce::Graphics::ScopedSaveState save (g);
        g.reduceClipRegion (juce::Rectangle<int> (550, 35, 352, 59));
        for (int x = 552; x < 902; x += 24)
        {
            g.setColour (cyan.withAlpha (0.055f)); g.drawVerticalLine (x, 36, 94);
        }
        for (int y = 40; y < 94; y += 20)
        {
            g.setColour (cyan.withAlpha (0.055f)); g.drawHorizontalLine (y, 550, 902);
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
            else if (oscType == 1) {
                v = 1 - 4 * std::abs (std::fmod (phase, 1.0f) - 0.5f);
                if(soundMode->box.getSelectedItemIndex()==0)v=std::round(v*7.5f)/7.5f;
            }
            else
            {
                const int step = i / 4;
                v = (float) ((step * 37 + step * step * 11) % 17 - 8) / 8;
            }
            const float x = 557 + (float) i / 384 * 338, y = 64 - v * 18;
            if (i == 0) waveform.startNewSubPath (x, y);
            else { waveform.lineTo (x, waveform.getCurrentPosition().y); waveform.lineTo (x, y); }
        }
        bloom (g, waveform, cyan.withAlpha (0.9f), 1.4f);
        for (int y = 36; y < 94; y += 3)
        {
            g.setColour (juce::Colours::black.withAlpha (0.1f)); g.drawHorizontalLine (y, 550, 902);
        }
    }

    const char* clockLabels[]={"FREE CLOCK","HOST STOP","HOST PLAY","FREE / NO HOST"};
    const int clock=juce::jlimit(0,3,instrument.clockStatus());
    g.setColour(inset.withAlpha(.9f));g.fillRoundedRectangle(400,81,140,15,3);
    caption(g,clockLabels[clock],{400,81,140,15},clock==2?cyan:muted,8.5f,true);

    panelShape(g,{1112,24,184,112});caption(g,"SOUND LAYERS",{1128,32,152,18},ink,10.f,true);
    section(g,{1112,160,184,648},"06",selectedLayer==0?"LAYER A":"LAYER B",selectedLayer==0?mint:violet);
    g.setColour(line);g.drawHorizontalLine(529,1128,1280);
    caption(g,"MASTER FX",{1128,536,96,22},amber,9.f,true);
    caption(g,effectsOn.getToggleState()?"SAT > HOLD > QUANT":"DRY / BYPASSED",{1128,781,156,18},effectsOn.getToggleState()?amber:muted,8.f,true);
    section (g, { 24, 160, 420, 170 }, "01", selectedLayer==0?"OSCILLATOR / A":"OSCILLATOR / B", selectedLayer==0?mint:violet);
    section (g, { 460, 160, 636, 170 }, "02", selectedLayer==0?"AMPLITUDE / A":"AMPLITUDE / B", amber);
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
    section (g, { 24, 546, 1072, 262 }, "05", selectedLayer==0?"SEQUENCES / A":"SEQUENCES / B", cyan);
    caption (g, choices[sequenceClock]->box.getSelectedItemIndex()==0?"60 Hz":"MUSICAL", { 1015, 555, 62, 20 }, cyan, 10, true);
    caption (g, "TEXT", { 40, 743, 56, 31 }, muted, 10, true);
    section (g, { 24, 818, 1272, 150 }, "07", "PRESETS / RANDOM", violet);
    caption (g, "SEED", { 520, 838, 60, 14 }, violet, 8.5f, true);
    caption (g, "LOCK SECTIONS", { 720, 838, 160, 14 }, violet, 8.5f, true);
    caption (g, "PLAY", { 25, 968, 60, 18 }, mint, 10, true);
    caption (g, "MIDI input or click the keys", { 81, 968, 280, 18 }, muted, 10);
    caption (g, "SCROLL TO CHANGE OCTAVE", { 882, 968, 214, 18 }, muted, 9, true);
    g.setColour (line); g.drawHorizontalLine (1057, 24, (float)getWidth()-24.f);
    caption (g, "REBIT RUST", { 24, 1061, 140, 15 }, muted, 8.5f, true);
    caption (g, "PULSE / TRIANGLE / NOISE", { 426, 1061, 300, 15 }, muted, 8.5f, true);
    caption (g, juce::String ("v") + JucePlugin_VersionString, { getWidth()-72, 1061, 48, 15 }, muted, 8.5f, true);
}

void RebitEditor::resized()
{
    if(playbackDisplay)playbackDisplay->setBounds(550,35,530,91);
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
    vibTab.setBounds(850,355,60,24);lfoTab.setBounds(916,355,54,24);macroTab.setBounds(976,355,92,24);
    for(int i=0;i<2;++i)if(lfoShape[(size_t)i]&&lfoRate[(size_t)i]){
        const int x=532+i*268;
        lfoShape[(size_t)i]->setBounds(x,387,150,45);
        lfoRate[(size_t)i]->setBounds(x+158,387,100,109);
        lfoRetrigger[(size_t)i].setBounds(x,470,150,22);
    }
    for(int i=0;i<4;++i)if(macroKnobs[(size_t)i])macroKnobs[(size_t)i]->setBounds(532+i*100,390,100,100);
    mpeOn.setBounds(945,395,140,22);if(mpeBend)mpeBend->setBounds(960,420,100,90);
    arpTab.setBounds(190,355,52,24);monoTab.setBounds(248,355,58,24);modTab.setBounds(312,355,56,24);arpEnabled.setBounds(390,355,98,24);
    for(size_t i=0;i<routeRows.size();++i)if(routeRows[i])routeRows[i]->setBounds(40,385+(int)i*34,446,32);
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
    keyboard->setBounds (24, 993, getWidth()-48, 54);
    editA.setBounds(1124,57,76,27);editB.setBounds(1210,57,76,27);
    layerAOn.setBounds(1124,98,76,22);layerBOn.setBounds(1210,98,76,22);
    for(size_t i=0;i<4;++i)if(layerKnobs[i])layerKnobs[i]->setBounds(1120+(int)(i%2)*88,208+(int)(i/2)*130,88,113);
    if(soundMode)soundMode->setBounds(1128,470,152,46);
    effectsOn.setBounds(1215,536,78,22);
    for(size_t i=0;i<4;++i)if(effectKnobs[i])effectKnobs[i]->setBounds(1120+(int)(i%2)*88,568+(int)(i/2)*102,88,87);
    presetBox.setBounds(40,855,360,32);
    presetPrev.setBounds(408,855,42,32);presetNext.setBounds(456,855,42,32);
    presetStatus.setBounds(40,893,450,20);
    seedEditor.setBounds(520,855,110,30);newSeed.setBounds(638,855,64,30);
    for(int i=0;i<6;++i)randomLocks[(size_t)i].setBounds(720+i*62,855,58,22);
    randomizeButton.setBounds(1110,849,130,34);undoButton.setBounds(1110,889,130,30);
    randomStatus.setBounds(520,893,570,20);
}
