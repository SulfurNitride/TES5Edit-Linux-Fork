object FrameUpdateAuthor: TFrameUpdateAuthor
  Left = 0
  Top = 0
  Width = 505
  Height = 293
  TabOrder = 0
  object StaticText1: TStaticText
    Left = 0
    Top = 0
    Width = 505
    Height = 25
    Align = alTop
    AutoSize = False
    Caption = 
      'Update Export Info data in NiHeader. Enter * to not update that ' +
      'particular field.'
    TabOrder = 0
  end
  object edAuthor: TLabeledEdit
    Left = 16
    Top = 48
    Width = 225
    Height = 23
    EditLabel.Width = 37
    EditLabel.Height = 15
    EditLabel.Caption = 'Author'
    TabOrder = 1
    Text = '*'
  end
  object edProcessScript: TLabeledEdit
    Left = 16
    Top = 104
    Width = 225
    Height = 23
    EditLabel.Width = 73
    EditLabel.Height = 15
    EditLabel.Caption = 'Process Script'
    TabOrder = 2
    Text = 'Sniff'
  end
  object edExportScript: TLabeledEdit
    Left = 256
    Top = 104
    Width = 225
    Height = 23
    EditLabel.Width = 67
    EditLabel.Height = 15
    EditLabel.Caption = 'Export Script'
    TabOrder = 3
    Text = '*'
  end
end
