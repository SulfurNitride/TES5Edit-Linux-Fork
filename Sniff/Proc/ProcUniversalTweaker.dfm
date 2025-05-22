object FrameUniversalTweaker: TFrameUniversalTweaker
  Left = 0
  Top = 0
  Width = 475
  Height = 280
  Hint = 
    'When checked also process descendants of specified block types, ' +
    'for example for NiNode it would be BSFadeNode, etc.'
  TabOrder = 0
  DesignSize = (
    475
    280)
  object Label1: TLabel
    Left = 16
    Top = 23
    Width = 443
    Height = 50
    Anchors = [akLeft, akTop, akRight]
    AutoSize = False
    Caption = 
      'Block types to process separated by comma if several. Or a path ' +
      'to the block if contains "\" which separates block name or type,' +
      ' for example "\BSFadeNode\arms2:2\NiAlphaProperty". When empty p' +
      'rocesses all blocks. Not used for material files.'
    WordWrap = True
  end
  object StaticText1: TStaticText
    Left = 0
    Top = 0
    Width = 475
    Height = 17
    Align = alTop
    AutoSize = False
    Caption = 
      'Change any element in defined block(s) of nif/kf and FO4 materia' +
      'l files. '
    TabOrder = 0
  end
  object edPath: TLabeledEdit
    Left = 16
    Top = 127
    Width = 203
    Height = 23
    Hint = 
      'Path examples: scale value in transformation structure "Transfor' +
      'm\Scale", first texture in texture set "Textures\[0]". You can c' +
      'heck elements names by converting to JSON format using Convert t' +
      'o JSON operation.'
    EditLabel.Width = 104
    EditLabel.Height = 15
    EditLabel.Caption = 'Path to the element'
    ParentShowHint = False
    ShowHint = True
    TabOrder = 1
    Text = 'Alpha'
  end
  object edValue: TLabeledEdit
    Left = 338
    Top = 127
    Width = 121
    Height = 23
    Anchors = [akLeft, akTop, akRight]
    EditLabel.Width = 28
    EditLabel.Height = 15
    EditLabel.Caption = 'Value'
    TabOrder = 2
    Text = '0.8'
  end
  object chkOldValueCheck: TCheckBox
    Left = 101
    Top = 164
    Width = 118
    Height = 17
    Caption = 'only if old value'
    TabOrder = 3
    OnClick = chkOldValueCheckClick
  end
  object cmbOldValueMode: TComboBox
    Left = 225
    Top = 160
    Width = 107
    Height = 23
    Style = csDropDownList
    DropDownCount = 20
    ItemIndex = 0
    TabOrder = 4
    Text = '='
    Items.Strings = (
      '='
      '<>'
      '>'
      '<'
      'Contains'
      'Doesn'#39't contain'
      'Starts with'
      'Ends with'
      'AND &'
      'NOT AND &')
  end
  object edOldValue: TEdit
    Left = 338
    Top = 160
    Width = 121
    Height = 23
    Anchors = [akLeft, akTop, akRight]
    TabOrder = 5
  end
  object cmbNewValueMode: TComboBox
    Left = 225
    Top = 127
    Width = 107
    Height = 23
    Style = csDropDownList
    DropDownCount = 20
    ItemIndex = 0
    TabOrder = 6
    Text = 'Set'
    Items.Strings = (
      'Set'
      'Add'
      'Mul'
      'Replace with'
      'Prepend'
      'Append')
  end
  object edBlocks: TEdit
    Left = 16
    Top = 79
    Width = 313
    Height = 23
    Anchors = [akLeft, akTop, akRight]
    TabOrder = 7
    Text = 'NiMaterialProperty'
  end
  object chkInherited: TCheckBox
    Left = 338
    Top = 81
    Width = 121
    Height = 17
    Hint = 
      'When checked also process descendants of specified block types, ' +
      'for example for NiNode it would be BSFadeNode, BSLeadAnimNode, B' +
      'SOrderedNode, etc.'
    Anchors = [akTop, akRight]
    Caption = 'and descendants'
    ParentShowHint = False
    ShowHint = True
    TabOrder = 8
  end
  object Button1: TButton
    Left = 16
    Top = 160
    Width = 73
    Height = 25
    Caption = 'Help'
    TabOrder = 9
    OnClick = Button1Click
  end
  object chkReport: TCheckBox
    Left = 16
    Top = 198
    Width = 257
    Height = 17
    Caption = 'Report only, don'#39't save anything'
    TabOrder = 10
  end
end
