Feature: Language
  As someone who'd rather read Japanese
  I want to pick the language of Threadbear's own words in Settings
  So the app speaks my language, while scripts and the activity log still know each control by its English name

  Background:
    Given the app is connected to a runner with the demo threads

  # Only the Japanese copy has the Language row (desktop/ja/README.md), and the
  # Swift app is frozen, so it never gets it (AGENTS.md). The scripted build
  # starts in English whatever the Mac's language is (desktop/ja/start.rs).
  # A kit button's screen reader name is the words it draws, so its label is
  # what's on screen; a control's tooltip stays its English name.
  @settings.general @swift-pending
  Scenario: Picking 日本語 draws Settings in Japanese, and its controls keep their English names
    When I open Settings
    Then Settings' language is "system" on an English screen
    And Settings draws the button "Play"
    When I pick "日本語" from Settings' language menu
    Then Settings' language is "ja" on a Japanese screen
    And Settings draws the button "鳴らす"
    When I press "Use Plum as the accent" in Settings
    Then Settings says the accent is "plum"
    When I pick "English" from Settings' language menu
    Then Settings' language is "en" on an English screen
    And Settings draws the button "Play"
