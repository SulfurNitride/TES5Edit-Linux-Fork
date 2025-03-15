{******************************************************************************

  This Source Code Form is subject to the terms of the Mozilla Public License,
  v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain
  one at https://mozilla.org/MPL/2.0/.

*******************************************************************************}

unit wbDefinitionsTES3;

{$I wbDefines.inc}

interface

procedure DefineTES3;

implementation

uses
  SysUtils,
  wbDefinitionsCommon,
  wbDefinitionsSignatures,
  wbInterface;

var
  wbAttributeEnum,
  wbDialogTypeEnum,
  wbMagicEffectEnum,
  wbSkillEnum,
  wbSpecializationEnum: IwbEnumDef;

  wbLeveledFlags: IwbFlagsDef;

  wbAIDT,
  wbBipedObjects,
  wbDELE,
  wbDESC,
  wbEffects,
  wbENAM,
  wbFNAM,
  wbITEX,
  wbItems,
  wbMODL,
  wbNAME,
  wbPackages,
  wbSCRI,
  wbSpells,
  wbTravelServices: IwbRecordMemberDef;

const
  wbKnownSubRecordSignaturesNoFNAM : TwbKnownSubRecordSignatures = (
    'NAME',
    '____',
    '____',
    '____',
    '____'
  );

  wbKnownSubRecordSignaturesLAND : TwbKnownSubRecordSignatures = (
    '____',
    '____',
    '____',
    'INTV',
    '____'
  );

  wbKnownSubRecordSignaturesREFR : TwbKnownSubRecordSignatures = (
    '____',
    '____',
    'NAME',
    '____',
    '____'
  );

  wbKnownSubRecordSignaturesINFO : TwbKnownSubRecordSignatures = (
    'INAM',
    'NAME',
    '____',
    '____',
    '____'
  );

  wbKnownSubRecordSignaturesINDX : TwbKnownSubRecordSignatures = (
    'INDX',
    'DESC',
    '____',
    '____',
    '____'
  );

  wbKnownSubRecordSignaturesSCPT : TwbKnownSubRecordSignatures = (
    'SCHD',
    '____',
    '____',
    '____',
    '____'
  );

  wbKnownSubRecordSignaturesSSCR : TwbKnownSubRecordSignatures = (
    'DATA',
    'NAME',
    '____',
    '____',
    '____'
  );

function wbCalcPGRCSize(aBasePtr: Pointer; aEndPtr: Pointer; const aElement: IwbElement): Cardinal;
  function ExtractCountFromLabel(const aElement: IwbElement; aCount: Integer): Integer;
  begin
    var i := Pos('#', aElement.Name);
    if i = 0 then
      Result := aCount
    else try
      Result := StrToInt(Trim(Copy(aElement.Name, i+1, Length(aElement.Name))))+1;
    except
      Result := aCount;
    end;
  end;
begin
  var Index := ExtractCountFromLabel(aElement, aElement.Container.ElementCount);
  Result := ((aElement.Container.Container as IwbMainRecord).RecordBySignature['PGRP'].Elements[Pred(Index)] as IwbContainer).Elements[2].NativeValue;
end;

function wbDialogDataDecider(aBasePtr: Pointer; aEndPtr: Pointer; const aElement: IwbElement): Integer;
begin
  Result := 0;
  if Assigned(aElement) then
    if aElement.DataSize = 4 then
      Result := 1;
end;

function wbFRMRToString(aInt: Int64; const aElement: IwbElement; aType: TwbCallbackType): string;
begin
  Result := '';
  if aType in [ctToStr, ctToSummary, ctToSortKey, ctToEditValue] then begin
    Result := IntToHex(aInt, 8);
    if aType = ctToEditValue then
      Result := '$' + Result;
  end;
end;

function wbGridCellToFormID(aFormIDBase: Byte; const aGridCell: TwbGridCell; out aFormID: TwbFormID): Boolean;
begin
  Result := False;
  with aGridCell do begin
    if (x < -512) or (x > 511) or (y < -512) or (y > 511) then
      Exit;
    aFormID := TwbFormID.FromCardinal((Cardinal(x + 512) shl 10) + Cardinal(y + 512) + (Cardinal(aFormIDBase) shl 16));
    Result := True;
  end;
end;

function wbNPCDataDecider(aBasePtr: Pointer; aEndPtr: Pointer; const aElement: IwbElement): Integer;
begin
  Result := 0;
  if Assigned(aElement) then
    if aElement.DataSize = 12 then
      Result := 1;
end;

function wbSkillDecider(aBasePtr: Pointer; aEndPtr: Pointer; const aElement: IwbElement): Integer;
var
  Container : IwbContainer;
  INDX      : IwbElement;
begin
  Result := 0;
  if not wbTryGetContainerFromUnion(aElement, Container) then
    Exit;

  Container := Container.Container;
  if not Assigned(Container) then
    Exit;

  INDX := Container.ElementBySignature['INDX'];
  if not Assigned(INDX) then
    Exit;

  var i := INDX.NativeValue;
  case i of
    1: Result := 1;
    2,3,17,21: Result := 2;
    4,5,6,7,22,23,26: Result := 3;
    8: Result := 4;
    9: Result := 5;
    10,11,12,13,14,15: Result := 6;
    16: Result := 7;
    18: Result := 8;
    19: Result := 9;
    20: Result := 10;
    24: Result := 11;
    25: Result := 12;
  end;
end;

function wbConditionDecider(aBasePtr: Pointer; aEndPtr: Pointer; const aElement: IwbElement): Integer;
var
  Container : IwbContainer;
begin
  Result := 0;
  if not wbTryGetContainerFromUnion(aElement, Container) then
    Exit;

  case (Container.ElementNativeValues['Type']) of
    $32, $33, $43: Result := 1;
    $34, $35, $36, $37, $38, $39, $41, $42: Result := 2;
  end;
end;

procedure wbCELLAfterLoad(const aElement: IwbElement);
var
  Container  : IwbContainerElementRef;
  MainRecord : IwbMainRecord;
begin
  if wbBeginInternalEdit then try
    if not wbTryGetContainerWithValidMainRecord(aElement, Container, MainRecord) then
      Exit;

    if (Container.ElementNativeValues['DATA\Flags'] and 1) <> 0 then begin
      if not Assigned(MainRecord.ElementBySignature['WHGT']) then begin
        Container.ElementNativeValues['WHGT'] := Container.ElementNativeValues['INTV'];
        Container.RemoveElement('INTV');
      end;
    end;
  finally
    wbEndInternalEdit;
  end;
end;

procedure wbDELEAfterLoad(const aElement: IwbElement);
var
  Container  : IwbContainerElementRef;
  MainRecord : IwbMainRecord;
begin
  if wbBeginInternalEdit then try
    if not wbTryGetContainerWithValidMainRecord(aElement, Container, MainRecord) then
      Exit;

    if Assigned(MainRecord.ElementBySignature['DELE']) then
      Container.ElementNativeValues['DELE'] := 0;
  finally
    wbEndInternalEdit;
  end;
end;

//There are several errors in the Morrowind.esm with invalid values for FLTV.
procedure wbGlobalAfterLoad(const aElement: IwbElement);
var
  Container  : IwbContainerElementRef;
  MainRecord : IwbMainRecord;
begin
  if wbBeginInternalEdit then try
    if not wbTryGetContainerWithValidMainRecord(aElement, Container, MainRecord) then
      Exit;

    if Assigned(MainRecord.ElementBySignature['FLTV']) then
      if MainRecord.ElementBySignature['FNAM'].Value = 'Short' then //Only occurs on shorts.
        if (MainRecord.ElementBySignature['FLTV'].NativeValue = -92233720368547758.1) or
           (MainRecord.ElementBySignature['FLTV'].NativeValue = 0.04) or
           (MainRecord.ElementBySignature['FLTV'].Value = 'NaN') then
          Container.ElementNativeValues['FLTV'] := 0; //All errors are zero in the CS.
  finally
    wbEndInternalEdit;
  end;
end;

procedure wbFactionReactionToStr(var aValue: string; aBasePtr: Pointer; aEndPtr: Pointer; const aElement: IwbElement; aType: TwbCallbackType);
var
  Container: IwbContainerElementRef;
begin
  if not wbTrySetContainer(aElement, aType, Container) then
    Exit;

  var Faction := Container.Elements[0];
  var Reaction := Container.Elements[1].NativeValue;
  aValue := IntToStr(Reaction) + ' ' + Faction.Value;
  if Reaction >= 0 then
    aValue := '+' + aValue;
end;

//Work around for forwarding bug when a wbString is forwarded inside a member def.
procedure wbForwardForRealz(const aElement: IwbElement);
var
  Container : IwbContainer;
  Element   : IwbElement;
begin
  if wbBeginInternalEdit then try
    if Length(aElement.Value) > 0 then begin
      Container := aElement.Container;
      Element := Container.ElementbyName['Target'];
      if not Assigned(Element) then
        Element := Container.ElementbyName['Sound'];
        if not Assigned(Element) then
          Exit;

        var i := 1;
        while i <= Length(aElement.Value) do begin
          if aElement.Value[i] = AnsiChar(#0) then
            Break;
          Inc(i);
        end;
        var s := Copy(aElement.Value, 0, i);
        Element.NativeValue := s;
     end;
  finally
    wbEndInternalEdit;
  end;
end;

procedure wbINGRAfterLoad(const aElement: IwbElement);
var
  Container  : IwbContainerElementRef;
  MainRecord : IwbMainRecord;
begin
  if wbBeginInternalEdit then try
    if not wbTryGetContainerWithValidMainRecord(aElement, Container, MainRecord) then
      Exit;

    var i : integer;
    for i := 0 to 3 do begin
      case (Container.ElementNativeValues['IRDT\Effects\Magic Effects\Magic Effect #' + IntToStr(i)]) of
        17, 22, 74, 79: Container.ElementNativeValues['IRDT\Effects\Skills\Skill #' + IntToStr(i)] := -1;
        21, 26, 78, 83: Container.ElementNativeValues['IRDT\Effects\Attributes\Attribute #' + IntToStr(i)] := -1;
      else
        Container.ElementNativeValues['IRDT\Effects\Skills\Skill #' + IntToStr(i)] := -1;
        Container.ElementNativeValues['IRDT\Effects\Attributes\Attribute #' + IntToStr(i)] := -1;
      end;
    end;
  finally
    wbEndInternalEdit;
  end;
end;

procedure wbTES3Header(const aElement: IwbElement);
var
  Container  : IwbContainerElementRef;
  MainRecord : IwbMainRecord;
begin
  if wbBeginInternalEdit then try
    if not wbTryGetContainerWithValidMainRecord(aElement, Container, MainRecord) then
      Exit;

      if Assigned(MainRecord.ElementBySignature['HEDR']) then
        if (Container.ElementNativeValues['HEDR\Record Flags'] and 1) = 1 then
          MainRecord.SetIsESM(True);
  finally
    wbEndInternalEdit;
  end;
end;

procedure DefineTES3;
begin
  DefineCommon;
  wbHeaderSignature := 'TES3';

  wbRecordFlags := wbInteger('Record Flags', itU32, wbFlags(['ESM']));

  wbMainRecordHeader := wbStruct('Record Header', [
    wbString('Signature', 4, cpCritical),
    wbInteger('Data Size', itU32, nil, cpIgnore),
    wbByteArray('Version Control Info', 4, cpIgnore).SetToStr(wbVCI1ToStrBeforeFO4),
    wbRecordFlags
  ]);

  wbSizeOfMainRecordStruct := 16;

  wbKnownSubRecordSignatures[ksrEditorID] := 'NAME';
  wbKnownSubRecordSignatures[ksrFullName] := 'FNAM';
  wbKnownSubRecordSignatures[ksrBaseRecord] := '____';
  wbKnownSubRecordSignatures[ksrGridCell] := 'DATA';

  {>>> Enums <<<}

  wbAttributeEnum :=
    wbEnum([
      {0} 'Strength',
      {1} 'Intelligence',
      {2} 'Willpower',
      {3} 'Agility',
      {4} 'Speed',
      {5} 'Endurance',
      {6} 'Personality',
      {7} 'Luck'
    ], [
      -1, 'None'
    ]);

  wbDialogTypeEnum :=
    wbEnum([
      {0} 'Regular Topic',
      {1} 'Voice',
      {2} 'Greeting',
      {3} 'Persuasion',
      {4} 'Journal'
    ]);

  wbMagicEffectEnum :=
    wbEnum([
        {0} 'Water Breathing',
        {1} 'Swift Swim',
        {2} 'Water Walking',
        {3} 'Shield',
        {4} 'Fire Shield',
        {5} 'Lightning Shield',
        {6} 'Frost Shield',
        {7} 'Burden',
        {8} 'Feather',
        {9} 'Jump',
       {10} 'Levitate',
       {11} 'Slow Fall',
       {12} 'Lock',
       {13} 'Open',
       {14} 'Fire Damage',
       {15} 'Shock Damage',
       {16} 'Frost Damage',
       {17} 'Drain Attribute',
       {18} 'Drain Health',
       {19} 'Drain Magicka',
       {20} 'Drain Fatigue',
       {21} 'Drain Skill',
       {22} 'Damage Attribute',
       {23} 'Damage Health',
       {24} 'Damage Magicka',
       {25} 'Damage Fatigue',
       {26} 'Damage Skill',
       {27} 'Poison',
       {28} 'Weakness To Fire',
       {29} 'Weakness To Frost',
       {30} 'Weakness To Shock',
       {31} 'Weakness To Magicka',
       {32} 'Weakness To Common Disease',
       {33} 'Weakness To Blight Disease',
       {34} 'Weakness To Corprus Disease',
       {35} 'Weakness To Poison',
       {36} 'Weakness To Normal Weapons',
       {37} 'Disintegrate Weapon',
       {38} 'Disintegrate Armor',
       {39} 'Invisibility',
       {40} 'Chameleon',
       {41} 'Light',
       {42} 'Sanctuary',
       {43} 'Night Eye',
       {44} 'Charm',
       {45} 'Paralyze',
       {46} 'Silence',
       {47} 'Blind',
       {48} 'Sound',
       {49} 'Calm Humanoid',
       {50} 'Calm Creature',
       {51} 'Frenzy Humanoid',
       {52} 'Frenzy Creature',
       {53} 'Demoralize Humanoid',
       {54} 'Demoralize Creature',
       {55} 'Rally Humanoid',
       {56} 'Rally Creature',
       {57} 'Dispel',
       {58} 'Soultrap',
       {59} 'Telekinesis',
       {60} 'Mark',
       {61} 'Recall',
       {62} 'Divine Intervention',
       {63} 'Almsivi Intervention',
       {64} 'Detect Animal',
       {65} 'Detect Enchantment',
       {66} 'Detect Key',
       {67} 'Spell Absorption',
       {68} 'Reflect',
       {69} 'Cure Common Disease',
       {70} 'Cure Blight Disease',
       {71} 'Cure Corprus Disease',
       {72} 'Cure Poison',
       {73} 'Cure Paralyzation',
       {74} 'Restore Attribute',
       {75} 'Restore Health',
       {76} 'Restore Magicka',
       {77} 'Restore Fatigue',
       {78} 'Restore Skill',
       {79} 'Fortify Attribute',
       {80} 'Fortify Health',
       {81} 'Fortify Magicka',
       {82} 'Fortify Fatigue',
       {83} 'Fortify Skill',
       {84} 'Fortify Maximum Magicka',
       {85} 'Absorb Attribute',
       {86} 'Absorb Health',
       {87} 'Absorb Magicka',
       {88} 'Absorb Fatigue',
       {89} 'Absorb Skill',
       {90} 'Resist Fire',
       {91} 'Resist Frost',
       {92} 'Resist Shock',
       {93} 'Resist Magicka',
       {94} 'Resist Common Disease',
       {95} 'Resist Blight Disease',
       {96} 'Resist Corprus Disease',
       {97} 'Resist Poison',
       {98} 'Resist Normal Weapons',
       {99} 'Resist Paralysis',
      {100} 'Remove Curse',
      {101} 'Turn Undead',
      {102} 'Summon Scamp',
      {103} 'Summon Clannfear',
      {104} 'Summon Daedroth',
      {105} 'Summon Dremora',
      {106} 'Summon Ancestral Ghost',
      {107} 'Summon Skeletal Minion',
      {108} 'Summon Least Bonewalker',
      {109} 'Summon Greater Bonewalker',
      {110} 'Summon Bonelord',
      {111} 'Summon Winged Twilight',
      {112} 'Summon Hunger',
      {113} 'Summon Golden Saint',
      {114} 'Summon Flame Atronach',
      {115} 'Summon Frost Atronach',
      {116} 'Summon Storm Atronach',
      {117} 'Fortify Attack Bonus',
      {118} 'Command Creature',
      {119} 'Command Humanoid',
      {120} 'Bound Dagger',
      {121} 'Bound Longsword',
      {122} 'Bound Mace',
      {123} 'Bound Battle Axe',
      {124} 'Bound Spear',
      {125} 'Bound Longbow',
      {126} 'Unused 126',
      {127} 'Bound Cuirass',
      {128} 'Bound Helm',
      {129} 'Bound Boots',
      {130} 'Bound Shield',
      {131} 'Bound Gloves',
      {132} 'Corpus',
      {133} 'Vampirism',
      {134} 'Summon Centurion Sphere',
      {135} 'Sun Damage',
      {136} 'Stunted Magicka',
      {137} 'Summon Fabricant',
      {138} 'Call Wolf',
      {139} 'Call Bear',
      {140} 'Summon Bonewolf',
      {141} 'Unused 141',
      {142} 'Unused 142'
    ], [
       -1, 'None'
    ]);

  wbSkillEnum :=
    wbEnum([
       {0} 'Block',
       {1} 'Armorer',
       {2} 'Medium Armor',
       {3} 'Heavy Armor',
       {4} 'Blunt Weapon',
       {5} 'Long Blade',
       {6} 'Axe',
       {7} 'Spear',
       {8} 'Athletics',
       {9} 'Enchant',
      {10} 'Destruction',
      {11} 'Alteration',
      {12} 'Illusion',
      {13} 'Conjuration',
      {14} 'Mysticism',
      {15} 'Restoration',
      {16} 'Alchemy',
      {17} 'Unarmored',
      {18} 'Security',
      {19} 'Sneak',
      {20} 'Acrobatics',
      {21} 'Light Armor',
      {22} 'Short Blade',
      {23} 'Marksman',
      {24} 'Mercantile',
      {25} 'Speechcraft',
      {26} 'Hand-to-Hand'
    ], [
      -1, 'None'
    ]);

  wbSpecializationEnum :=
    wbEnum([
      {0} 'Combat',
      {1} 'Magic',
      {2} 'Stealth'
    ]);

  {>>> Flags <<<}

  wbLeveledFlags :=
    wbFlags([
      {0} 'Calculate from all levels <= player''s level',
      {1} 'Calculate for each item in count'
    ]);

  {>>> Common Defs <<<}

  wbDELE := wbInteger(DELE, 'Deleted', itU32, wbEnum(['True']));
  wbDESC := wbString(DESC, 'Description');
  wbFNAM := wbString(FNAM, 'Name');
  wbITEX := wbString(ITEX, 'Icon Filename');
  wbMODL := wbString(MODL, 'Model Filename');
  wbNAME := wbString(NAME, 'Editor ID').SetRequired.IncludeFlag(dfSummarySelfAsShortName);
  wbSCRI := wbString(SCRI, 'Script'); //[SCPT]
  wbENAM := wbString(ENAM, 'Enchanment'); //[ENCH]

  {>>> Record Members <<<}

  wbAIDT :=
    wbStruct(AIDT, 'AI Data', [
      wbInteger('Hello', itU16).SetDefaultNativeValue(30), //0 CREA
      wbInteger('Fight', itU8).SetDefaultNativeValue(30), //90 CREA
      wbInteger('Flee', itU8).SetDefaultNativeValue(30), //20 CREA
      wbInteger('Alarm', itU8),
      wbUnused(3),
      wbInteger('Buy/Sell Service Flags', itU32, wbServiceFlags).IncludeFlag(dfCollapsed, wbCollapseFlags)
    ]).SetRequired;

  wbBipedObjects :=
    wbRArrayS('Biped Objects',
      wbRStructSK([0], 'Biped Object', [
        wbInteger(INDX, 'Body Part', itU8, wbEnum([
           {0} 'Head',
           {1} 'Hair',
           {2} 'Neck',
           {3} 'Chest',
           {4} 'Groin',
           {5} 'Skirt',
           {6} 'Right Hand',
           {7} 'Left Hand',
           {8} 'Right Wrist',
           {9} 'Left Wrist',
          {10} 'Shield',
          {11} 'Right Forearm',
          {12} 'Left Forearm',
          {13} 'Right Upper Arm',
          {14} 'Left Upper Arm',
          {15} 'Right Foot',
          {16} 'Left Foot',
          {17} 'Right Ankle',
          {18} 'Left Ankle',
          {19} 'Right Knee',
          {20} 'Left Knee',
          {21} 'Right Upper Leg',
          {22} 'Left Upper Leg',
          {23} 'Right Pauldron',
          {24} 'Left Pauldron',
          {25} 'Weapon',
          {26} 'Tail'
        ])),
        wbString(BNAM, 'Male'), //[BODY]
        wbString(CNAM, 'Female') //[BODY]
      ]).SetSummaryKey([0, 1, 2])
        .SetSummaryMemberPrefixSuffix(0, 'Part: ', ',')
        .SetSummaryMemberPrefixSuffix(1, 'Male: ', ',')
        .SetSummaryMemberPrefixSuffix(2, 'Female: ', '')
        .IncludeFlag(dfSummaryMembersNoName)
        .IncludeFlag(dfSummaryNoSortKey)
        .IncludeFlag(dfCollapsed, wbCollapseBodyParts));

  wbEffects :=
    wbRArray('Effects',
      wbStructSK(ENAM, [0], 'Effect', [
        wbInteger('Magic Effect', itU16, wbMagicEffectEnum).SetDefaultNativeValue(-1),
        wbInteger('Skill', itS8, wbSkillEnum).SetDefaultNativeValue(-1),
        wbInteger('Attribute', itS8, wbAttributeEnum).SetDefaultNativeValue(-1),
        wbInteger('Range', itU32,
          wbEnum([
            {0} 'Self',
            {1} 'Touch',
            {2} 'Target'
          ])),
        wbInteger('Area', itU32),
        wbInteger('Duration', itU32),
        wbInteger('Magnitude Minimum', itU32),
        wbInteger('Magnitude Maximum', itU32)
      ]).SetSummaryKeyOnValue([0, 3, 1, 2])
        .SetSummaryPrefixSuffixOnValue(0, 'MGEF: ', ',')
        .SetSummaryPrefixSuffixOnValue(3, 'Range: ', ',')
        .SetSummaryPrefixSuffixOnValue(1, 'Skill: ', ',')
        .SetSummaryPrefixSuffixOnValue(2, 'Attribute: ', '')
        .IncludeFlag(dfSummaryMembersNoName)
        .IncludeFlag(dfSummaryNoSortKey));

  wbItems :=
    wbRArrayS('Item Entries',
      wbStructSK(NPCO, [1], 'Item Entry', [
        wbInteger('Count', itS32),
        wbString('Item', 32) //[ALCH, APPA, ARMO, BOOK, CLOT, INGR, LEVI, LIGH, LOCK, MISC, PROB, REPA, WEAP]
      ]).SetSummaryKeyOnValue([1, 0])
        .SetSummaryPrefixSuffixOnValue(0, 'x', '}')
        .SetSummaryPrefixSuffixOnValue(1, '{', '')
        .IncludeFlag(dfCollapsed, wbCollapseItems));

  wbPackages :=
    wbRArray('AI Packages',
      wbRUnion('AI Packages', [
        wbStruct(AI_W, 'AI Wander', [
          wbInteger('Distance', itU16).SetDefaultNativeValue(512),
          wbInteger('Duration (Hours)', itU16).SetDefaultNativeValue(5),
          wbInteger('Time of Day', itU8),
          wbStruct('Idle Chances', [
            wbInteger('Idle 2', itU8).SetDefaultNativeValue(60),
            wbInteger('Idle 3', itU8).SetDefaultNativeValue(20),
            wbInteger('Idle 4', itU8).SetDefaultNativeValue(10),
            wbInteger('Idle 5', itU8),
            wbInteger('Idle 6', itU8),
            wbInteger('Idle 7', itU8),
            wbInteger('Idle 8', itU8),
            wbInteger('Idle 9', itU8)
          ]),
          wbInteger('Reset', itU8, wbBoolEnum).SetDefaultNativeValue(1)
        ]),
        wbStruct(AI_T, 'AI Travel', [
          wbVec3('Location'),
          wbInteger('Reset', itU8, wbBoolEnum).SetDefaultNativeValue(1),
          wbUnused(3)
        ]),
        wbStruct(AI_E, 'AI Escort', [
          wbVec3('Specific Point'),
          wbInteger('Duration (Hours)', itU16),
          wbString(True, 'Target', 32).SetAfterLoad(wbForwardForRealz), //[CREA, NPC_]
          wbInteger('Reset', itU16, wbBoolEnum).SetDefaultNativeValue(1)
        ]),
        wbStruct(AI_F, 'AI Follow', [
          wbVec3('Specific Point'),
          wbInteger('Duration (Hours)', itU16),
          wbString(True, 'Target', 32).SetAfterLoad(wbForwardForRealz), //[CREA, NPC_]
          wbInteger('Reset', itU16, wbBoolEnum).SetDefaultNativeValue(1)
        ]),
        wbStruct(AI_A, 'AI Activate', [
          wbString(True, 'Target', 32).SetAfterLoad(wbForwardForRealz), //[ACTI, ALCH, APPA, ARMO, BODY, BOOK, CLOT, CONT, CREA, DOOR, ENCH, INGR, LIGH, LEVC, LEVI, LOCK, MISC, NPC_, PROB, REPA, SPEL, STAT, WEAP]
          wbInteger('Reset', itU8, wbBoolEnum).SetDefaultNativeValue(1)
        ]),
        wbString(CNDT, 'Escort/Follow To Cell') //[CELL]
      ]));

  wbSpells := wbRArrayS('Spells', wbString(NPCS, 'Spell', 32)); //[SPEL]

  wbTravelServices :=
    wbRArray('Travel Services',
      wbRStruct('Travel Service', [
        wbVec3PosRot(DODT, 'Destination'),
        wbStringForward(DNAM, 'Cell', 64)
      ]));

  {>>> Records <<<}

  wbRecord(TES3, 'Main File Header', [
    wbStruct(HEDR, 'Header', [
      wbFloat('Version', cpNormal, False, 1, 2).SetDefaultNativeValue(1.3),
      wbRecordFlags,
      wbString('Author', 32),
      wbString('Description', 256),
      wbInteger('Number of Records', itU32)
    ]).SetRequired,
    wbRArray('Master Files',
      wbRStruct('Master File', [
        wbStringForward(MAST, 'Master Filename').SetRequired,
        wbInteger(DATA, 'Master Size', itU64, nil, cpIgnore, True)
    ])).IncludeFlag(dfInternalEditOnly, not wbAllowMasterFilesEdit)
  ], False, nil, cpNormal, True)
    .SetGetFormIDCallback(function(const aMainRecord: IwbMainRecord; out aFormID: TwbFormID): Boolean begin
       Result := True;
       aFormID := TwbFormID.Null;
     end)
     .SetAfterLoad(wbTES3Header);

  wbRecord(ACTI, 'Activator', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM.SetRequired,
    wbSCRI
  ]).SetFormIDBase($40);

  wbRecord(ALCH, 'Potion', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL,
    wbString(TEXT, 'Icon Filename'),
    wbSCRI,
    wbFNAM,
    wbStruct(ALDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2),
      wbInteger('Potion Value', itU32),
      wbInteger('Auto Calculate Value', itU32, wbBoolEnum).SetDefaultNativeValue(True)
    ]).SetRequired,
    wbEffects
  ]).SetFormIDBase($40);

  wbRecord(APPA, 'Alchemical Apparatus', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM.SetRequired,
    wbSCRI,
    wbStruct(AADT, 'Data', [
      wbInteger('Type', itU32,
        wbEnum([
          {0} 'Mortar & Pestle',
          {1} 'Alembic',
          {2} 'Calcinator',
          {3} 'Retort'
        ])).SetDefaultNativeValue(1),
      wbFloat('Quality', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1)
    ]).SetRequired,
    wbITEX
  ]).SetFormIDBase($40);

  wbRecord(ARMO, 'Armor', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM.SetRequired,
    wbSCRI,
    wbStruct(AODT, 'Data', [
      wbInteger('Type', itU32,
        wbEnum([
           {0} 'Helmet',
           {1} 'Cuirass',
           {2} 'Left Pauldron',
           {3} 'Right Pauldron',
           {4} 'Greaves',
           {5} 'Boots',
           {6} 'Left Gauntlet',
           {7} 'Right Gauntlet',
           {8} 'Shield',
           {9} 'Left Bracer',
          {10} 'Right Bracer'
        ])).SetDefaultNativeValue(5),
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1),
      wbInteger('Health', itU32).SetDefaultNativeValue(100),
      wbInteger('Enchantment Charge', itU32).SetDefaultNativeValue(100),
      wbInteger('Armor Rating', itU32).SetDefaultNativeValue(1)
    ]).SetRequired,
    wbITEX,
    wbBipedObjects,
    wbENAM
  ]).SetFormIDBase($40);

  wbRecord(BODY, 'Body Part', @wbKnownSubRecordSignaturesNoFNAM, wbFlags(wbFlagsList([
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbString(FNAM, 'Skin Race').SetDefaultNativeValue('Argonian').SetRequired, //[RACE]
    wbStruct(BYDT, 'Data', [
      wbInteger('Body Part', itU8,
        wbEnum([
           {0} 'Head',
           {1} 'Hair',
           {2} 'Neck',
           {3} 'Chest',
           {4} 'Groin',
           {5} 'Hand',
           {6} 'Wrist',
           {7} 'Forearm',
           {8} 'Upperarm',
           {9} 'Foot',
          {10} 'Ankle',
          {11} 'Knee',
          {12} 'Upperleg',
          {13} 'Clavicle',
          {14} 'Tail'
        ])).SetDefaultNativeValue(10),
      wbInteger('Skin Type', itU8,
        wbEnum([
          {0} 'Normal',
          {1} 'Vampire'
        ])),
      wbInteger('Flags', itU8,
        wbFlags([
          {0} 'Female',
          {1} 'Not Playable'
        ])).IncludeFlag(dfCollapsed, wbCollapseFlags),
      wbInteger('Part Type', itU8,
        wbEnum([
          {0} 'Skin',
          {1} 'Clothing',
          {2} 'Armor'
        ]))
    ]).SetRequired
  ]).SetFormIDBase($20).SetSummaryKey([2]);

  wbRecord(BOOK, 'Book', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM,
    wbStruct(BKDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1),
      wbInteger('Scroll', itU32, wbBoolEnum),
      wbInteger('Teaches', itS32, wbSkillEnum).SetDefaultNativeValue(-1),
      wbInteger('Enchantment Charge', itU32).SetDefaultNativeValue(100)
    ]).SetRequired,
    wbSCRI,
    wbITEX,
    wbStringKC(TEXT, 'Book Text', 0, cpTranslate),
    wbENAM
  ]).SetFormIDBase($40);

  wbRecord(BSGN, 'Birthsign', [
    wbDELE,
    wbNAME,
    wbFNAM.SetRequired(False),
    wbString(TNAM, 'Constellation Filename'),
    wbDESC,
    wbSpells
  ]).SetFormIDBase($10);

  wbRecord(CELL, 'Cell', [
    wbString(NAME, 'Location').SetRequired,
    wbDELE,
    wbStruct(DATA, 'Data', [
      wbInteger('Flags', itU32,
        wbFlags(wbSparseFlags([
          0, 'Is Interior Cell',
          1, 'Has Water',
          2, 'Illegal To Sleep Here',
          6, 'Has Map Color',
          7, 'Behave Like Exterior'
        ], False, 8))).IncludeFlag(dfCollapsed, wbCollapseFlags),
      wbStruct('Grid', [
        wbInteger('X', itS32),
        wbInteger('Y', itS32)
      ]).SetSummaryKey([0, 1])
        .SetSummaryMemberPrefixSuffix(0, '(', '')
        .SetSummaryMemberPrefixSuffix(1, '', ')')
        .SetSummaryDelimiter(', ')
        .SetDontShow(wbCellInteriorDontShow)
    ]).SetRequired,
    wbInteger(INTV, 'Water Height', itS32, nil, cpIgnore).SetDontShow(wbCellExteriorDontShow),
    wbString(RGNN, 'Region'),  //[REGN]
    wbByteColors(NAM5, 'Region Map Color').SetDontShow(wbCellInteriorDontShow),
    wbFloat(WHGT, 'Water Height').SetDontShow(wbCellExteriorDontShow),
    wbStruct(AMBI, 'Ambience', [
      wbByteColors('Ambient Color'), //71, 71, 71, 0 RGBATAG
      wbByteColors('Sunlight Color'), //242, 217, 217, 0 RGBATAG
      wbByteColors('Fog Color'),
      wbFloat('Fog Density', cpNormal, False, 1, 2).SetDefaultNativeValue(1)
    ]).SetDontShow(wbCellExteriorDontShow)
  ]).SetFormIDBase($B0)
    .SetGetGridCellCallback(function(const aSubRecord: IwbSubRecord; out aGridCell: TwbGridCell): Boolean begin
      with aGridCell, aSubRecord do begin
        Result := not (ElementNativeValues['Flags\Is Interior Cell'] = True);
        if Result then begin
          X := ElementNativeValues['Grid\X'];
          Y := ElementNativeValues['Grid\Y'];
        end;
      end;
    end)
    .SetGetFormIDCallback(function(const aMainRecord: IwbMainRecord; out aFormID: TwbFormID): Boolean begin
      var GridCell: TwbGridCell;
      Result := aMainRecord.GetGridCell(GridCell) and wbGridCellToFormID($A0, GridCell, aFormID);
    end)
    .SetIdentityCallback(function(const aMainRecord: IwbMainRecord): string begin
      var GridCell: TwbGridCell;
      if aMainRecord.GetGridCell(GridCell) then
        Result := '<Exterior>' + GridCell.SortKey
      else
        Result := aMainRecord.EditorID;
    end)
    .SetAfterLoad(wbCELLAfterLoad);

  wbRecord(CLAS, 'Class', [
    wbNAME,
    wbDELE,
    wbFNAM.SetRequired,
    wbStruct(CLDT, 'Data', [
      wbArray('Primary Attributes', wbInteger('Attribute', itS32, wbAttributeEnum), 2),
      wbInteger('Specialization', itU32, wbSpecializationEnum),
      wbArray('Major & Minor Skill Sets',
        wbStruct('Skill Set', [
          wbInteger('Minor Skill', itS32, wbSkillEnum),
          wbInteger('Major Skill', itS32, wbSkillEnum)
        ]), 5),
      wbInteger('Playable', itU32, wbBoolEnum),
      wbInteger('Buy/Sell Service Flags', itU32, wbServiceFlags).IncludeFlag(dfCollapsed, wbCollapseFlags)
    ]).SetRequired,
    wbDESC
  ]).SetFormIDBase($18);

  wbRecord(CLOT, 'Clothing', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM,
    wbStruct(CTDT, 'Data', [
      wbInteger('Type', itU32, wbEnum([
        {0} 'Pants',
        {1} 'Shoes',
        {2} 'Shirt',
        {3} 'Belt',
        {4} 'Robe',
        {5} 'Right Glove',
        {6} 'Left Glove',
        {7} 'Skirt',
        {8} 'Ring',
        {9} 'Amulet'
      ])).SetDefaultNativeValue(9),
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU16).SetDefaultNativeValue(1),
      wbInteger('Enchantment Charge', itU16).SetDefaultNativeValue(100)
    ]).SetRequired,
    wbSCRI,
    wbITEX,
    wbBipedObjects,
    wbENAM
  ]).SetFormIDBase($40);

  wbRecord(CONT, 'Container', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM,
    wbFloat(CNDT, 'Weight', cpNormal, False, 1, 2).SetRequired,
    wbInteger(FLAG, 'Flags', itU32,
      wbFlags(wbSparseFlags([
        0, 'Organic',
        1, 'Respawns',
        3, 'Can Hold Items'
      ], False, 4))).SetDefaultNativeValue(4).IncludeFlag(dfCollapsed, wbCollapseFlags).SetRequired,
    wbSCRI,
    wbItems
  ]).SetFormIDBase($40);

  wbRecord(CREA, 'Creature', wbFlags(wbFlagsList([
      10, 'Corpses Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbString(CNAM, 'Sound Generator Creature'), //[CREA]
    wbFNAM,
    wbSCRI,
    wbStruct(NPDT, 'Data', [
      wbInteger('Type', itU32,
        wbEnum([
          {0} 'Creature',
          {1} 'Daedra',
          {2} 'Undead',
          {3} 'Humanoid'
        ])),
      wbInteger('Level', itU32).SetDefaultNativeValue(1),
      wbStruct('Attributes', [
        wbInteger('Strength', itU32).SetDefaultNativeValue(50),
        wbInteger('Intelligence', itU32).SetDefaultNativeValue(50),
        wbInteger('Willpower', itU32).SetDefaultNativeValue(50),
        wbInteger('Agility', itU32).SetDefaultNativeValue(50),
        wbInteger('Speed', itU32).SetDefaultNativeValue(50),
        wbInteger('Endurance', itU32).SetDefaultNativeValue(50),
        wbInteger('Personality', itU32).SetDefaultNativeValue(50),
        wbInteger('Luck', itU32).SetDefaultNativeValue(50)
      ]),
      wbInteger('Health', itU32).SetDefaultNativeValue(50),
      wbInteger('Magicka', itU32).SetDefaultNativeValue(50),
      wbInteger('Fatigue', itU32).SetDefaultNativeValue(50),
      wbInteger('Soul', itU32).SetDefaultNativeValue(50),
      wbStruct('Skills', [
        wbInteger('Combat', itU32).SetDefaultNativeValue(50),
        wbInteger('Magic', itU32).SetDefaultNativeValue(50),
        wbInteger('Stealth', itU32).SetDefaultNativeValue(50)
      ]),
      wbArray('Attack Sets',
        wbStruct('Attack Set', [
          wbInteger('Minimum', itU32).SetDefaultNativeValue(1),
          wbInteger('Maximum', itU32).SetDefaultNativeValue(5)
        ]), 3),
      wbInteger('Barter Gold', itU32)
    ]).SetRequired,
    wbInteger(FLAG, 'Flags', itU32,
      wbFlags(wbSparseFlags([
         0, 'Biped',
         1, 'Respawn',
         2, 'Weapon & Shield',
         3, 'Can Hold Items',
         4, 'Swims',
         5, 'Flies',
         6, 'Walks',
         7, 'Essential',
        10, 'Skeleton Blood',
        11, 'Metal Blood'
      ], False, 12))).SetDefaultNativeValue(48).IncludeFlag(dfCollapsed, wbCollapseFlags).SetRequired,
    wbFloat(XSCL, 'Scale', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
    wbItems,
    wbSpells,
    wbAIDT,
    wbTravelServices,
    wbPackages
  ]).SetFormIDBase($40);

  wbRecord(DIAL, 'Dialog Topic', [
    wbNAME,
    wbUnion(DATA, 'Data', wbDialogDataDecider, [
      wbInteger('Dialog Type', itU8, wbDialogTypeEnum),
      wbInteger('Dialog Type', itU32, wbDialogTypeEnum)
    ]).SetRequired,
    wbDELE
  ]).SetFormIDBase($80).SetSummaryKey([1]);

  wbRecord(DOOR, 'Door', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL,
    wbFNAM.SetRequired(False),
    wbSCRI,
    wbString(SNAM, 'Open Sound'), //[SOUN]
    wbString(ANAM, 'Close Sound') //[SOUN]
  ]).SetFormIDBase($40);

  wbRecord(ENCH, 'Enchantment', wbFlags(wbFlagsList([
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbStruct(ENDT, 'Data', [
      wbInteger('Cast Type', itU32,
        wbEnum([
          {0} 'Cast Once',
          {1} 'Cast Strikes',
          {2} 'Cast When Used',
          {3} 'Constant Effect'
        ])),
      wbInteger('Enchantment Cost', itU32),
      wbInteger('Charge Amount', itU32),
      wbInteger('Auto Calculate', itS16,
        wbEnum([
          {0} 'False',
          {1} 'True'
        ], [
          -2, 'Constant Effect',
          -1, 'Not Applicable'
        ])),
      wbUnused(2)
    ]).SetRequired,
    wbEffects
  ]).SetFormIDBase($04).SetSummaryKey([3]);

  wbRecord(FACT, 'Faction', [
    wbNAME,
    wbDELE,
    wbFNAM.SetRequired,
    wbRArray('Rank Titles', wbStringForward(RNAM, 'Rank Title', 32)),
    wbStruct(FADT, 'Data', [
      wbArray('Favored Attributes', wbInteger('Attribute', itS32, wbAttributeEnum), 2),
      wbArray('Rank Requirements',
        wbStruct('Rank Requirement', [
          wbInteger('Favored Attribute 1', itS32),
          wbInteger('Favored Attribute 2', itS32),
          wbInteger('Primary Skills', itS32),
          wbInteger('Favored Skills', itS32),
          wbInteger('Faction Reputation', itU32)
        ]).SetSummaryKey([0, 1, 2, 3, 4])
          .SetSummaryMemberPrefixSuffix(0, 'Attribute 0:', ',')
          .SetSummaryMemberPrefixSuffix(1, 'Attribute 2:', ',')
          .SetSummaryMemberPrefixSuffix(2, 'Primary Skills:', ',')
          .SetSummaryMemberPrefixSuffix(3, 'Favored Skills:', ',')
          .SetSummaryMemberPrefixSuffix(4, 'Faction Reputation:', '')
          .IncludeFlag(dfSummaryMembersNoName)
          .IncludeFlag(dfCollapsed), 10),
      wbArray('Favored Skills', wbInteger('Favored Skill', itS32, wbSkillEnum), 7),
      wbInteger('Hidden From Player', itU32, wbBoolEnum)
    ]).SetRequired,
    wbRArrayS('Relations',
      wbRStructSK([0], 'Relation', [
        wbString(ANAM, 'Faction'), //[FACT]
        wbInteger(INTV, 'Reaction', itS32)
      ]).SetToStr(wbFactionReactionToStr))
  ]).SetFormIDBase($1C);

  wbRecord(GLOB, 'Global', @wbKnownSubRecordSignaturesNoFNAM, [
    wbNAME,
    wbDELE,
    wbInteger(FNAM, 'Variable Type', itU8,
      wbEnum([], [
        $66, 'Float',
        $6C, 'Long',
        $73, 'Short'
      ])).SetDefaultNativeValue($73),
    wbFloat(FLTV, 'Value', cpNormal, False, 1, 2)
  ]).SetFormIDBase($58).SetSummaryKey([3]).SetAfterLoad(wbGlobalAfterLoad);

  wbRecord(GMST, 'Game Setting', [
    wbNAME,
    wbRUnion('Value', [
      wbString(STRV, 'String Value'),
      wbInteger(INTV, 'Integer Value', itS32),
      wbFloat(FLTV, 'Float Value', cpNormal, False, 1, 4)
    ])
  ]).SetFormIDBase($50).SetSummaryKey([1]).IncludeFlag(dfIndexEditorID);

  wbRecord(INFO, 'Dialog Response', @wbKnownSubRecordSignaturesINFO, [
    wbString(INAM, 'Response ID').SetRequired,
    wbString(PNAM, 'Previous Response ID').SetRequired,
    wbString(NNAM, 'Next Response ID').SetRequired,
    wbStruct(DATA, 'Data', [
      wbInteger('Dialog Type', itU32, wbDialogTypeEnum),
      wbInteger('Disposition/Index', itU32),
      wbInteger('Speaker Faction Rank', itS8).SetDefaultNativeValue(-1),
      wbInteger('Sex', itS8, wbSexEnum).SetDefaultNativeValue(-1),
      wbInteger('Player Faction Rank', itS8).SetDefaultNativeValue(-1),
      wbUnused(1)
    ]).SetRequired,
    wbString(ONAM, 'Speaker'), //[CREA, NPC_]
    wbString(RNAM, 'Speaker Race'), //[RACE]
    wbString(CNAM, 'Speaker Class'), //[CLAS]
    wbString(FNAM, 'Speaker Faction'), //[FACT]
    wbString(ANAM, 'Speaker Cell'), //[CELL]
    wbString(DNAM, 'Player Faction'), //[FACT]
    wbString(SNAM, 'Sound Filename'),
    wbString(NAME, 'Response'),
    wbDELE,
    wbRArray('Conditions',
      wbRStruct('Condition', [
        wbStruct(SCVR, 'Condition', [
          wbInteger('Position', itU8,
            wbEnum([], [
              $30, '1st',
              $31, '2nd',
              $32, '3rd',
              $33, '4th',
              $34, '5th',
              $35, '6th'
            ])),
          wbInteger('Type', itU8,
            wbEnum([], [
              $31, 'Function',
              $32, 'Global', //[GLOB]
              $33, 'Script Variable',
              $34, 'Journal',
              $35, 'Item',
              $36, 'Dead',
              $37, 'Not ID',
              $38, 'Not Faction',
              $39, 'Not Class',
              $41, 'Not Race',
              $42, 'Not Cell',
              $43, 'Not Script Variable'
            ])),
          wbUnion('Function', wbConditionDecider, [
            wbInteger('Function', itU16,
              wbEnum([], [
                $3030, 'Reaction Low',
                $3031, 'PC Strength',
                $3032, 'PC Enchant',
                $3033, 'PC Sneak',
                $3034, 'PC Common Disease',
                $3035, 'Choice',
                $3036, 'PC Vampire',
                $3037, 'Flee',
                $3130, 'Reaction High',
                $3131, 'PC Block',
                $3132, 'PC Destruction',
                $3133, 'PC Acrobatics',
                $3134, 'PC Blight Disease',
                $3135, 'PC Intelligence',
                $3136, 'Level',
                $3137, 'Should Attack',
                $3230, 'Rank Requirement',
                $3231, 'PC Armorer',
                $3232, 'PC Alteration',
                $3233, 'PC Light Armor',
                $3234, 'Clothing Modifier',
                $3235, 'PC Willpower',
                $3236, 'Attacked',
                $3237, 'Werewolf',
                $3330, 'Reputation',
                $3331, 'PC Medium Armor',
                $3332, 'PC Illusion',
                $3333, 'PC Short Blade',
                $3334, 'PC Crime Level',
                $3335, 'PC Agility',
                $3336, 'Talked To PC',
                $3337, 'PC Werewolf Kills',
                $3430, 'Health Percent',
                $3431, 'PC Heavy Armor',
                $3432, 'PC Conjuration',
                $3433, 'PC Marksman',
                $3434, 'Same Sex',
                $3435, 'PC Speed',
                $3436, 'PC Health',
                $3530, 'PC Reputation',
                $3531, 'PC Blunt Weapon',
                $3532, 'PC Mysticism',
                $3533, 'PC Mercantile',
                $3534, 'Same Race',
                $3535, 'PC Endurance',
                $3536, 'Creature Target',
                $3630, 'PC Level',
                $3631, 'PC Long Blade',
                $3632, 'PC Restoration',
                $3633, 'PC Speechcraft',
                $3634, 'Same Faction',
                $3635, 'PC Personality',
                $3636, 'Friend Hit',
                $3730, 'PC Health Percent',
                $3731, 'PC Axe',
                $3732, 'PC Alchemy',
                $3733, 'PC Hand To Hand',
                $3734, 'Faction Rank Difference',
                $3735, 'PC Luck',
                $3736, 'Fight',
                $3830, 'PC Magicka',
                $3831, 'PC Spear',
                $3832, 'PC Unarmored',
                $3833, 'PC Sex',
                $3834, 'Detected',
                $3835, 'PC Corpus',
                $3836, 'Hello',
                $3930, 'PC Fatigue',
                $3931, 'PC Athletics',
                $3932, 'PC Security',
                $3933, 'PC Expelled',
                $3934, 'Alarmed',
                $3935, 'Weather',
                $3936, 'Alarm'
              ])),
            wbInteger('Variable Type', itU16,
              wbEnum([], [
                $5866, 'Float',
                $586C, 'Long',
                $5873, 'Short'
              ])),
            wbInteger('Object Type', itU16,
              wbEnum([], [
                $5843, 'Class', //[CLAS]
                $5844, 'Actor', //[CREA, NPC_]
                $5846, 'Faction', //[FACT]
                $5849, 'Inventory Item', //[ALCH, APPA, ARMO, BOOK, CLOT, INGR, LIGH, LOCK, MISC, PROB, REPA, WEAP]
                $584A, 'Dialog Topic', //DIAL
                $584C, 'Cell', //[CELL]
                $5852, 'Race', //[RACE]
                $5858, 'Non-Player Character' //[NPC_]
              ]))
            ]),
          wbInteger('Operator', itU8,
            wbEnum([], [
              $30, 'Equal To',
              $31, 'Not Equal To',
              $32, 'Less Than',
              $33, 'Less Than or Equal To',
              $34, 'Greater Than',
              $35, 'Greater Than or Equal To'
            ])),
          wbString('Variable/Object')
        ]),
        wbRUnion('Value', [
          wbInteger(INTV, 'Value', itS32),
          wbFloat(FLTV, 'Value')
        ])
      ])),
    wbRStruct('Quest Data', [
      wbInteger(QSTN, 'Quest Named', itU8, wbBoolEnum).SetDefaultNativeValue(1),
      wbInteger(QSTF, 'Quest Finished', itU8, wbBoolEnum).SetDefaultNativeValue(1),
      wbInteger(QSTR, 'Quest Restarted', itU8, wbBoolEnum).SetDefaultNativeValue(1)
    ], [], cpNormal, False, nil, True),
    wbString(BNAM, 'Result')
  ]).SetFormIDBase($90);

  wbRecord(INGR, 'Ingredient', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM,
    wbStruct(IRDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1),
      wbStruct('Effects', [
        wbArray('Magic Effects', wbInteger('Magic Effect', itS32, wbMagicEffectEnum).SetDefaultNativeValue(-1), 4),
        wbArray('Skills', wbInteger('Skill', itS32, wbSkillEnum).SetDefaultNativeValue(-1), 4),
        wbArray('Attributes', wbInteger('Attribute', itS32, wbAttributeEnum).SetDefaultNativeValue(-1), 4)
      ])
    ]).SetRequired,
    wbSCRI,
    wbITEX
  ]).SetFormIDBase($40).SetAfterLoad(wbINGRAfterLoad);

  wbRecord(LAND, 'Landscape', @wbKnownSubRecordSignaturesLAND, [
    wbStruct(INTV, 'Grid', [
      wbInteger('X', itS32),
      wbInteger('Y', itS32)
    ], cpCritical).SetSummaryKeyOnValue([0, 1])
                  .SetSummaryPrefixSuffixOnValue(0, '(', '')
                  .SetSummaryPrefixSuffixOnValue(1, '', ')')
                  .SetSummaryDelimiterOnValue(', ')
                  .SetRequired,
    wbInteger(DATA, 'Flags', itU32,
      wbFlags([
        {0} 'Has Vertex Normals/Height Map',
        {1} 'Has Vertex Colors',
        {2} 'Has Landscape Textures',
        {3} 'User Created/Edited'
      ])).SetDefaultNativeValue(8).IncludeFlag(dfCollapsed, wbCollapseFlags),
    IfThen(wbSimpleRecords,
      wbByteArray(VNML, 'Vertex Normals'),
      wbArray(VNML, 'Vertex Normals',
        wbArray('Row',
          wbStruct('Column', [
            wbInteger('X', itS8, nil, cpBenign, False, nil, nil, 0, wbLandNormalsGetCP),
            wbInteger('Y', itS8, nil, cpBenign, False, nil, nil, 0, wbLandNormalsGetCP),
            wbInteger('Z', itS8, nil, cpBenign, False, nil, nil, 0, wbLandNormalsGetCP)
          ]).SetSummaryKey([0, 1, 2])
            .SetSummaryMemberPrefixSuffix(0, '' + '(', '')
            .SetSummaryMemberPrefixSuffix(2, '', ')')
            .IncludeFlag(dfSummaryMembersNoName)
            .IncludeFlag(dfCollapsed, wbCollapseVec3),
        65).SetSummaryName('Columns').IncludeFlag(dfCollapsed),
      65).SetSummaryName('Rows').IncludeFlag(dfCollapsed)),
    IfThen(wbSimpleRecords,
      wbByteArray(VHGT, 'Vertex Height Map'),
      wbStruct(VHGT, 'Vertex Height Map', [
        wbFloat('Offset'),
        wbUnused(1),
        wbArray('Height Map', wbArray('Row', wbInteger('Column', itS8),
          65).SetSummaryName('Columns').IncludeFlag(dfCollapsed),
        65).SetSummaryName('Rows').IncludeFlag(dfCollapsed),
        wbUnused(2)
      ])),
    IfThen(wbSimpleRecords,
      wbByteArray(WNAM, 'World Map Colors'),
      wbArray(WNAM, 'World Map Colors',
        wbArray('Row', wbInteger('Column', itS8),
        9).SetSummaryName('Columns').IncludeFlag(dfCollapsed),
      9).SetSummaryName('Rows').IncludeFlag(dfCollapsed)),
    IfThen(wbSimpleRecords,
      wbByteArray(VCLR, 'Vertex Colors'),
      wbArray(VCLR, 'Vertex Colors',
        wbArray('Row', wbByteColorsRGB('Column').SetToStr(wbRGBAToStr).IncludeFlag(dfCollapsed, wbCollapseVec3),
        65).SetSummaryName('Columns').IncludeFlag(dfCollapsed),
      65).SetSummaryName('Rows').IncludeFlag(dfCollapsed)),
    IfThen(wbSimpleRecords,
      wbByteArray(VTEX, 'Landscape Textures'),
      wbArray(VTEX, 'Landscape Textures', wbArray('Row', wbInteger('Column', itU16),
        16).SetSummaryName('Columns').IncludeFlag(dfCollapsed),
      16).SetSummaryName('Rows').IncludeFlag(dfCollapsed))
  ]).SetFormIDBase($D0)
    .SetFormIDNameBase($B0)
    .SetGetFormIDCallback(function(const aMainRecord: IwbMainRecord; out aFormID: TwbFormID): Boolean begin
      var GridCell: TwbGridCell;
      Result := aMainRecord.GetGridCell(GridCell) and wbGridCellToFormID($C0, GridCell, aFormID);
    end)
    .SetIdentityCallback(function(const aMainRecord: IwbMainRecord): string begin
      Result := '';
      var GridCell: TwbGridCell;
      if aMainRecord.GetGridCell(GridCell) then
        Result := GridCell.SortKey
    end);

  wbRecord(LEVC, 'Leveled Creature', wbFlags(wbFlagsList([
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbInteger(DATA, 'Flags', itU32, wbLeveledFlags).IncludeFlag(dfCollapsed, wbCollapseFlags).SetRequired,
    wbInteger(NNAM, 'Chance None %', itU8).SetRequired,
    wbInteger(INDX, 'Entry Count', itU32),
    wbRArrayS('Leveled Creature Entries',
      wbRStructSK([1], 'Leveled Creature Entry', [
        wbString(CNAM, 'Creature'), //[CREA]
        wbInteger(INTV, 'Player Level', itU16)
      ]).SetSummaryKey([1, 0])
        .SetSummaryMemberPrefixSuffix(1, '[Level: ', ']')
        .SetSummaryMemberPrefixSuffix(0, '', ' x1')
        .IncludeFlag(dfSummaryMembersNoName)
        .IncludeFlag(dfSummaryNoSortKey)
        .IncludeFlag(dfCollapsed, wbCollapseLeveledItems)
    ).SetCountPath(INDX)
  ]).SetFormIDBase($40).SetSummaryKey([5]);

  wbRecord(LEVI, 'Leveled Item', wbFlags(wbFlagsList([
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbInteger(DATA, 'Flags', itU32, wbLeveledFlags).IncludeFlag(dfCollapsed, wbCollapseFlags).SetRequired,
    wbInteger(NNAM, 'Chance None %', itU8).SetRequired,
    wbInteger(INDX, 'Entry Count', itU32),
    wbRArrayS('Leveled Item Entries',
      wbRStructSK([1], 'Leveled Item Entry', [
        wbString(INAM, 'Item'), //[ALCH, APPA, ARMO, BOOK, CLOT, INGR, LEVI, LIGH, LOCK, MISC, PROB, REPA, WEAP]
        wbInteger(INTV, 'Player Level', itU16)
      ]).SetSummaryKey([1, 0])
        .SetSummaryMemberPrefixSuffix(1, '[Level: ', ']')
        .SetSummaryMemberPrefixSuffix(0, '', ' x1')
        .IncludeFlag(dfSummaryMembersNoName)
        .IncludeFlag(dfSummaryNoSortKey)
        .IncludeFlag(dfCollapsed, wbCollapseLeveledItems)
    ).SetCountPath(INDX)
  ]).SetFormIDBase($40).SetSummaryKey([5]);

  wbRecord(LIGH, 'Light', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM,
    wbITEX,
    wbStruct(LHDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2),
      wbInteger('Value', itU32),
      wbInteger('Time', itS32).SetDefaultNativeValue(-1),
      wbInteger('Radius', itU32).SetDefaultNativeValue(1000),
      wbByteColors, //255, 255, 255, 0 RGBATAG
      wbInteger('Flags', itU32,
        wbFlags([
          {0} 'Dynamic',
          {1} 'Can Be Carried',
          {2} 'Negative',
          {3} 'Flicker',
          {4} 'Fire',
          {5} 'Off By Default',
          {6} 'Flicker Slow',
          {7} 'Pulse',
          {8} 'Pulse Slow'
        ])).IncludeFlag(dfCollapsed, wbCollapseFlags)
    ]).SetRequired,
    wbSCRI,
    wbString(SNAM, 'Looping Sound') //[SOUN]
  ]).SetFormIDBase($40);

  wbRecord(LOCK, 'Lockpick', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL,
    wbFNAM.SetRequired,
    wbStruct(LKDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1),
      wbFloat('Quality', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Uses', itU32).SetDefaultNativeValue(10)
    ]).SetRequired,
    wbSCRI,
    wbITEX
  ]).SetFormIDBase($40);

  wbRecord(LTEX, 'Landscape Texture', [
    wbDELE,
    wbNAME,
    wbInteger(INTV, 'Texture ID', itU32).SetRequired,
    wbString(DATA, 'Texture Filename').SetRequired
  ]).SetFormIDBase($60).SetSummaryKey([3]);

  wbRecord(MGEF, 'Magic Effect', @wbKnownSubRecordSignaturesINDX, [
    wbInteger(INDX, 'Effect', itU32, wbMagicEffectEnum),
    wbStruct(MEDT, 'Data', [
      wbInteger('School', itU32,
        wbEnum([
          {0} 'Alteration',
          {1} 'Conjuration',
          {2} 'Destruction',
          {3} 'Illusion',
          {4} 'Mysticism',
          {5} 'Restoration'
        ])),
      wbFloat('Base Cost', cpNormal, False, 1, 2),
      wbInteger('Flags', itU32,
        wbFlags(wbSparseFlags([
          9,  'Spellmaking',
          10, 'Enchanting',
          11, 'Negative'
        ]))).IncludeFlag(dfCollapsed, wbCollapseFlags),
      wbByteColorsInt32,
      wbFloat('Size Multiplier', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbFloat('Speed Multiplier', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbFloat('Size Cap', cpNormal, False, 1, 2)
    ]).SetRequired,
    wbITEX,
    wbString(PTEX, 'Particle Texture Filename'),
    wbString(BSND, 'Bolt Sound'), //[SOUN]
    wbString(CSND, 'Cast Sound'), //[SOUN]
    wbString(HSND, 'Hit Sound'), //[SOUN]
    wbString(ASND, 'Area Sound'), //[SOUN]
    wbString(CVFX, 'Casting Visual'), //[STAT]
    wbString(BVFX, 'Bolt Visual'), //[WEAP]
    wbString(HVFX, 'Hit Visual'), //[STAT]
    wbString(AVFX, 'Area Visual'), //[STAT]
    wbDESC
  ]).SetFormIDBase($02);

  wbRecord(MISC, 'Miscellaneous Item', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM,
    wbStruct(MCDT,'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1),
      wbInteger('Used As Key', itU32, wbBoolEnum) //This is true if the object is used as a key on a Reference.
    ]).SetRequired,
    wbSCRI,
    wbITEX
  ]).SetFormIDBase($40);

  wbRecord(NPC_, 'Non-Player Character', wbFlags(wbFlagsList([
      10, 'Corpses Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL,
    wbFNAM,
    wbString(RNAM, 'Race').SetDefaultNativeValue('Argonian').SetRequired, //[RACE]
    wbString(CNAM, 'Class').SetDefaultNativeValue('Acrobat').SetRequired, //[CLAS]
    wbString(ANAM, 'Faction').SetRequired, //[FACT]
    wbString(BNAM, 'Head Body Part').SetDefaultNativeValue('b_n_argonian_m_head_03').SetRequired, //[BODY]
    wbString(KNAM, 'Hair Body Part').SetDefaultNativeValue('b_n_argonian_m_hair02').SetRequired, //[BODY]
    wbSCRI,
    wbUnion(NPDT, 'Data', wbNPCDataDecider, [
      wbStruct('Stats', [
        wbInteger('Level', itS16).SetDefaultNativeValue(1),
        wbStruct('Attributes', [
          wbInteger('Strength', itU8).SetDefaultNativeValue(50),
          wbInteger('Intelligence', itU8).SetDefaultNativeValue(50),
          wbInteger('Willpower', itU8).SetDefaultNativeValue(50),
          wbInteger('Agility', itU8).SetDefaultNativeValue(50),
          wbInteger('Speed', itU8).SetDefaultNativeValue(50),
          wbInteger('Endurance', itU8).SetDefaultNativeValue(50),
          wbInteger('Personality', itU8).SetDefaultNativeValue(50),
          wbInteger('Luck', itU8).SetDefaultNativeValue(50)
        ]),
        wbStruct('Skills', [
          wbInteger('Block', itU8).SetDefaultNativeValue(5),
          wbInteger('Armorer', itU8).SetDefaultNativeValue(5),
          wbInteger('Medium Armor', itU8).SetDefaultNativeValue(5),
          wbInteger('Heavy Armor', itU8).SetDefaultNativeValue(5),
          wbInteger('Blunt Weapon', itU8).SetDefaultNativeValue(5),
          wbInteger('Long Blade', itU8).SetDefaultNativeValue(5),
          wbInteger('Axe', itU8).SetDefaultNativeValue(5),
          wbInteger('Spear', itU8).SetDefaultNativeValue(5),
          wbInteger('Athletics', itU8).SetDefaultNativeValue(5),
          wbInteger('Enchant', itU8).SetDefaultNativeValue(5),
          wbInteger('Destruction', itU8).SetDefaultNativeValue(5),
          wbInteger('Alteration', itU8).SetDefaultNativeValue(5),
          wbInteger('Illusion', itU8).SetDefaultNativeValue(5),
          wbInteger('Conjuration', itU8).SetDefaultNativeValue(5),
          wbInteger('Mysticism', itU8).SetDefaultNativeValue(5),
          wbInteger('Restoration', itU8).SetDefaultNativeValue(5),
          wbInteger('Alchemy', itU8).SetDefaultNativeValue(5),
          wbInteger('Unarmored', itU8).SetDefaultNativeValue(5),
          wbInteger('Security', itU8).SetDefaultNativeValue(5),
          wbInteger('Sneak', itU8).SetDefaultNativeValue(5),
          wbInteger('Acrobatics', itU8).SetDefaultNativeValue(5),
          wbInteger('Light Armor', itU8).SetDefaultNativeValue(5),
          wbInteger('Short Blade', itU8).SetDefaultNativeValue(5),
          wbInteger('Marksman', itU8).SetDefaultNativeValue(5),
          wbInteger('Speechcraft', itU8).SetDefaultNativeValue(5),
          wbInteger('Mercantile', itU8).SetDefaultNativeValue(5),
          wbInteger('Hand-to-Hand', itU8).SetDefaultNativeValue(5)
        ]),
        wbUnused(1),
        wbInteger('Health', itU16).SetDefaultNativeValue(50),
        wbInteger('Magicka', itU16).SetDefaultNativeValue(100),
        wbInteger('Fatigue', itU16).SetDefaultNativeValue(200),
        wbInteger('Disposition', itU8).SetDefaultNativeValue(50),
        wbInteger('Reputation', itU8),
        wbInteger('Rank', itU8),
        wbUnused(1),
        wbInteger('Gold', itU32)
      ]),
      wbStruct('Auto Calculated Stats', [
        wbInteger('Level', itU16).SetDefaultNativeValue(1),
        wbInteger('Disposition', itU8).SetDefaultNativeValue(50),
        wbInteger('Reputation', itU8),
        wbInteger('Rank', itU8),
        wbUnused(3),
        wbInteger('Gold', itU32)
      ])
    ]).SetRequired,
    wbInteger(FLAG, 'Flags', itU32,
      wbFlags(wbSparseFlags([
        0, 'Female',
        1, 'Essential',
        2, 'Respawn',
        3, 'Can Hold Items',
        4, 'Auto Calculate Stats',
        10, 'Skeleton Blood',
        11, 'Metal Blood'
      ], False, 12))).SetDefaultNativeValue(18).IncludeFlag(dfCollapsed, wbCollapseFlags),
    wbItems,
    wbSpells,
    wbAIDT,
    wbTravelServices,
    wbPackages,
    wbFloat(XSCL, 'Scale', cpNormal, False, 1, 2).SetDefaultNativeValue(1)
  ]).SetFormIDBase($40);

  wbRecord(PGRD, 'Path Grid', [
    wbStruct(DATA, 'Data', [
      wbStruct('Grid', [
        wbInteger('X', itS32),
        wbInteger('Y', itS32)
      ], cpCritical).SetSummaryKey([0, 1])
                    .SetSummaryMemberPrefixSuffix(0, '(', '')
                    .SetSummaryMemberPrefixSuffix(1, '', ')')
                    .SetSummaryDelimiter(', '),
      wbInteger('Granularity', itU16).SetDefaultNativeValue(1024),
      wbInteger('Point Count', itU16)
    ]).SetRequired,
    wbString(NAME, 'Location', 0, cpIgnore).SetRequired,
    IfThen(wbSimpleRecords,
      wbArray(PGRP, 'Points', wbByteArray('Point', 16)).SetCountPathOnValue('DATA\Point Count', False),
      wbArray(PGRP, 'Points',
        wbStruct('Point', [
          wbVec3PosInt32,
          wbInteger('User Created', itU8, wbBoolEnum),
          wbInteger('Connection Count', itU8),
          wbUnused(2)
        ]).SetSummaryKey([0, 2])
          .SetSummaryMemberPrefixSuffix(0, '', '')
          .SetSummaryMemberPrefixSuffix(2, 'Connections: ', '')
          .IncludeFlag(dfCollapsed)
        ).SetCountPathOnValue('DATA\Point Count', False)),
    IfThen(wbSimpleRecords,
      wbByteArray(PGRC, 'Point-to-Point Connections'),
      wbArray(PGRC, 'Point-to-Point Connections', wbArrayS('Point', wbInteger('Point', itU32), wbCalcPGRCSize)).IncludeFlag(dfCollapsed))
  ]).SetFormIDBase($F0)
    .SetFormIDNameBase($B0).SetGetGridCellCallback(function(const aSubRecord: IwbSubRecord; out aGridCell: TwbGridCell): Boolean begin
      with aGridCell, aSubRecord do begin
        X := ElementNativeValues['Grid\X'];
        Y := ElementNativeValues['Grid\Y'];
        Result := not ((X = 0) and (Y = 0));
      end;
    end)
    .SetGetFormIDCallback(function(const aMainRecord: IwbMainRecord; out aFormID: TwbFormID): Boolean begin
      var GridCell: TwbGridCell;
      Result := aMainRecord.GetGridCell(GridCell) and wbGridCellToFormID($E0, GridCell, aFormID);
    end)
    .SetIdentityCallback(function(const aMainRecord: IwbMainRecord): string begin
      var GridCell: TwbGridCell;
      if aMainRecord.GetGridCell(GridCell) then
        Result := '<Exterior>' + GridCell.SortKey
      else
        Result := aMainRecord.EditorID;
    end);

  wbRecord(PROB, 'Probe', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM,
    wbStruct(PBDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1),
      wbFloat('Quality', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Uses', itU32).SetDefaultNativeValue(10)
    ]).SetRequired,
    wbSCRI,
    wbITEX
  ]).SetFormIDBase($40);

  wbRecord(RACE, 'Race', [
    wbNAME,
    wbDELE,
    wbFNAM.SetRequired,
    wbStruct(RADT, 'Data', [
      wbArrayS('Skill Bonuses',
        wbStructSK([0], 'Skill Bonus', [
          wbInteger('Skill', itS32, wbSkillEnum),
          wbInteger('Bonus', itU32)
        ]).SetSummaryKey([1, 0])
          .SetSummaryMemberPrefixSuffix(1, '+', '')
          .SetSummaryMemberPrefixSuffix(0, '', '')
          .IncludeFlag(dfSummaryNoSortKey)
          .IncludeFlag(dfSummaryMembersNoName).IncludeFlag(dfCollapsed), 7),
      wbStruct('Base Attributes', [
        wbStruct('Strength', [
          wbInteger('Male', itU32).SetDefaultNativeValue(50),
          wbInteger('Female', itU32).SetDefaultNativeValue(50)
        ]),
        wbStruct('Intelligence', [
          wbInteger('Male', itU32).SetDefaultNativeValue(50),
          wbInteger('Female', itU32).SetDefaultNativeValue(50)
        ]),
        wbStruct('Willpower', [
          wbInteger('Male', itU32).SetDefaultNativeValue(50),
          wbInteger('Female', itU32).SetDefaultNativeValue(50)
        ]),
        wbStruct('Agility', [
          wbInteger('Male', itU32).SetDefaultNativeValue(50),
          wbInteger('Female', itU32).SetDefaultNativeValue(50)
        ]),
        wbStruct('Speed', [
          wbInteger('Male', itU32).SetDefaultNativeValue(50),
          wbInteger('Female', itU32).SetDefaultNativeValue(50)
        ]),
        wbStruct('Endurance', [
          wbInteger('Male', itU32).SetDefaultNativeValue(50),
          wbInteger('Female', itU32).SetDefaultNativeValue(50)
        ]),
        wbStruct('Personality', [
          wbInteger('Male', itU32).SetDefaultNativeValue(50),
          wbInteger('Female', itU32).SetDefaultNativeValue(50)
        ]),
        wbStruct('Luck', [
          wbInteger('Male', itU32).SetDefaultNativeValue(50),
          wbInteger('Female', itU32).SetDefaultNativeValue(50)
        ])
      ]),
      wbStruct('Height', [
        wbFloat('Male', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
        wbFloat('Female', cpNormal, False, 1, 2).SetDefaultNativeValue(1)
      ]),
      wbStruct('Weight', [
        wbFloat('Male', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
        wbFloat('Female', cpNormal, False, 1, 2).SetDefaultNativeValue(1)
      ]),
      wbInteger('Flags', itU32,
        wbFlags([
          {0} 'Playable',
          {1} 'Beast Race'
        ])).IncludeFlag(dfCollapsed, wbCollapseFlags)
    ]).SetRequired,
    wbSpells,
    wbDESC
  ]).SetFormIDBase($14);

  wbRecord(REFR, 'Placed Object', @wbKnownSubRecordSignaturesREFR, [
    wbStruct(CNDT, 'New Cell Owner', [
      wbInteger('X', itS32),
      wbInteger('Y', itS32)
    ]).SetSummaryKeyOnValue([0, 1])
      .SetSummaryPrefixSuffixOnValue(0, '(', '')
      .SetSummaryPrefixSuffixOnValue(1, '', ')')
      .SetSummaryDelimiterOnValue(', '),
    wbInteger(FRMR, 'Object Index', itU32, wbFRMRToString, nil, cpIgnore, True).IncludeFlag(dfInternalEditOnly).SetRequired,
    wbString(NAME, 'Base Object'), //[ACTI, ALCH, APPA, ARMO, BODY, BOOK, CLOT, CONT, CREA, DOOR, INGR, LEVC, LOCK, MISC, NPC_, PROB, REPA, STAT, WEAP]
    wbInteger(UNAM, 'Reference Blocked', itU8, wbEnum(['True'])),
    wbFloat(XSCL, 'Scale', cpNormal, False, 1, 2),
    wbRStructSK([], 'Owner Data', [
      wbString(ANAM, 'Owner'), //[NPC_]
      wbString(BNAM, 'Global Variable'), //[GLOB]
      wbString(CNAM, 'Faction Owner'), //[FACT]
      wbInteger(INDX, 'Faction Rank', itU32)
    ], [], cpNormal, False, nil, True),
    wbFloat(XCHG, 'Enchantment Charge', cpNormal, False, 1, 0),
    wbString(XSOL, 'Soul'), //[CREA]
    wbInteger(INTV, 'Health/Uses Left', itU32),
    wbInteger(NAM9, 'Count', itU32),
    wbRStructSK([], 'Teleport Destination', [
      wbVec3PosRot(DODT, 'Position/Rotation'),
      wbString(DNAM, 'Cell') //[CELL]
    ]),
    wbRStructSK([], 'Lock Data', [
      wbInteger(FLTV, 'Lock Level', itU32).SetRequired,
      wbString(KNAM, 'Key'), //[MISC]
      wbString(TNAM, 'Trap') //[ENCH]
    ], [], cpNormal, False, nil, True),
    wbDELE,
    wbVec3PosRot(DATA)
  ]).SetGetFormIDCallback(function(const aMainRecord: IwbMainRecord; out aFormID: TwbFormID): Boolean begin
      var lFRMR := aMainRecord.RecordBySignature[FRMR];
      Result := Assigned(lFRMR);
      if Result then begin
        aFormID := TwbFormID.FromCardinal(lFRMR.NativeValue);
        if aFormID.FileID.FullSlot = 0 then
          aFormID.FileID := TwbFileID.CreateFull($FF);
      end;
    end).SetAfterLoad(wbDELEAfterLoad);

  wbRecord(REGN, 'Region', [
    wbDELE,
    wbNAME,
    wbFNAM.SetRequired,
    wbStruct(WEAT, 'Weather Chances', [
      wbInteger('Clear', itU8).SetDefaultNativeValue(5),
      wbInteger('Cloudy', itU8).SetDefaultNativeValue(25),
      wbInteger('Foggy', itU8).SetDefaultNativeValue(35),
      wbInteger('Overcast', itU8).SetDefaultNativeValue(20),
      wbInteger('Rain', itU8).SetDefaultNativeValue(10),
      wbInteger('Thunder', itU8).SetDefaultNativeValue(5),
      wbInteger('Ash', itU8),
      wbInteger('Blight', itU8),
      wbInteger('Snow', itU8),
      wbInteger('Blizzard', itU8)
    ], cpNormal, True, nil, 8).SetRequired,
    wbString(BNAM, 'Sleep Creature'), //[LEVC]
    wbByteColors(CNAM, 'Region Map Color').SetRequired,
    wbRArrayS('Region Sounds',
      wbStructSK(SNAM, [0], 'Region Sound', [
        wbString(True, 'Sound', 32).SetAfterLoad(wbForwardForRealz), //[SOUN]
        wbInteger('Chance', itS8).SetDefaultNativeValue(50)
      ]).SetSummaryKeyOnValue([0, 1])
        .SetSummaryPrefixSuffixOnValue(0, 'Sound: ', ',')
        .SetSummaryPrefixSuffixOnValue(1, 'Chance: ', '').IncludeFlag(dfCollapsed))
  ]).SetFormIDBase($70);

  wbRecord(REPA, 'Repair Item', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM,
    wbStruct(RIDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1),
      wbInteger('Uses', itU32).SetDefaultNativeValue(10),
      wbFloat('Quality', cpNormal, False, 1, 2).SetDefaultNativeValue(1)
    ]).SetRequired,
    wbSCRI,
    wbITEX
  ]).SetFormIDBase($40);

  wbRecord(SCPT, 'Script', @wbKnownSubRecordSignaturesSCPT, [
    wbStruct(SCHD, 'Script Header', [
      wbString('Name', 32), //Can be saved with 36 Chars in the CS, but collides with # of Shorts.
      wbInteger('Number of Shorts', itU32),
      wbInteger('Number of Longs', itU32),
      wbInteger('Number of Floats', itU32),
      wbInteger('Compiled Size', itU32),
      wbInteger('Local Variable Size', itU32)
    ]).SetSummaryKeyOnValue([4, 5, 2, 1, 3])
      .SetSummaryPrefixSuffixOnValue(4, '{Compiled Size = ', ',')
      .SetSummaryPrefixSuffixOnValue(5, 'Local Var Size = ', ',')
      .SetSummaryPrefixSuffixOnValue(1, 'Shorts = ', ',')
      .SetSummaryPrefixSuffixOnValue(2, 'Longs = ', ',')
      .SetSummaryPrefixSuffixOnValue(3, 'Floats = ', '}')
      .IncludeFlagOnValue(dfSummaryMembersNoName)
      .IncludeFlag(dfCollapsed, wbCollapseScriptData).SetRequired,
    wbDELE,
    wbArrayS(SCVR, 'Script Variables', wbString('Script Variable', 0, cpCritical)),
    wbByteArray(SCDT, 'Compiled Script'),
    wbStringScript(SCTX, 'Script Source').SetRequired
  ]).SetFormIDBase($30)
    .SetGetEditorIDCallback(function (const aSubRecord: IwbSubRecord): string begin
      Result := aSubRecord.ElementEditValues['Name'];
    end)
    .SetSetEditorIDCallback(procedure (const aSubRecord: IwbSubRecord; const aEditorID: string) begin
      aSubRecord.ElementEditValues['Name'] := aEditorID;
    end)
    .SetToStr(wbScriptToStr);

  wbRecord(SKIL, 'Skill', @wbKnownSubRecordSignaturesINDX, [
    wbInteger(INDX, 'Name', itU32, wbSkillEnum).SetRequired,
    wbStruct(SKDT, 'Data', [
      wbInteger('Governing Attribute', itS32, wbAttributeEnum),
      wbInteger('Type', itU32, wbSpecializationEnum),
      wbUnion('Actions', wbSkillDecider, [
        wbStruct('Block', [
          wbFloat('Successful Block'),
          wbUnused(12)
        ]),
        wbStruct('Armorer', [
          wbFloat('Successful Repair'),
          wbUnused(12)
        ]),
        wbStruct('Armor', [
          wbFloat('Hit By Opponent'),
          wbUnused(12)
        ]),
        wbStruct('Weapon', [
          wbFloat('Successful Attack'),
          wbUnused(12)
        ]),
        wbStruct('Athletics', [
          wbFloat('Seconds of Running'),
          wbFloat('Seconds of Swimming'),
          wbUnused(8)
        ]),
        wbStruct('Enchant', [
          wbFloat('Recharge Item'),
          wbFloat('Use Magic Item'),
          wbFloat('Create Magic Item'),
          wbFloat('Cast When Strikes')
        ]),
        wbStruct('Magic School', [
          wbFloat('Successful Cast'),
          wbUnused(12)
        ]),
        wbStruct('Alchemy', [
          wbFloat('Potion Creation'),
          wbFloat('Ingredient Use'),
          wbUnused(8)
        ]),
        wbStruct('Security', [
          wbFloat('Defeat Trap'),
          wbFloat('Pick Lock'),
          wbUnused(8)
        ]),
        wbStruct('Sneak', [
          wbFloat('Avoid Notice'),
          wbFloat('Successful Pick-Pocket'),
          wbUnused(8)
        ]),
        wbStruct('Acrobatics', [
          wbFloat('Jump'),
          wbFloat('Fall'),
          wbUnused(8)
        ]),
        wbStruct('Mercantile', [
          wbFloat('Successful Bargain'),
          wbFloat('Successful Bribe'),
          wbUnused(8)
        ]),
        wbStruct('Speechcraft', [
          wbFloat('Successful Persuasion'),
          wbFloat('Failed Persuasion'),
          wbUnused(8)
        ])
      ])
    ]).SetRequired,
    wbDESC
  ]).SetFormIDBase($01);

  wbRecord(SNDG, 'Sound Generator', [
    wbNAME,
    wbInteger(DATA, 'Type', itU32,
      wbEnum([
        {0} 'Left Foot',
        {1} 'Right Foot',
        {2} 'Swim Left',
        {3} 'Swim Right',
        {4} 'Moan',
        {5} 'Roar',
        {6} 'Scream',
        {7} 'Land'
      ])).SetDefaultNativeValue(7).SetRequired,
    wbString(CNAM, 'Creature'), //[CREA]
    wbString(SNAM, 'Sound').SetDefaultNativeValue('Body Fall Medium').SetRequired, //[SOUN]
    wbDELE
  ]).SetFormIDBase($28).SetSummaryKey([3]);

  wbRecord(SOUN, 'Sound', [
    wbNAME,
    wbDELE,
    wbString(FNAM, 'Sound Filename').SetRequired,
    wbStruct(DATA, 'Data', [
      wbInteger('Volume', itU8, wbDiv(255, 2)).SetDefaultNativeValue(1),
      wbInteger('Minimum Range', itU8),
      wbInteger('Maximum Range', itU8)
    ]).SetRequired
  ]).SetFormIDBase($40);

  wbRecord(SPEL, 'Spell', wbFlags(wbFlagsList([
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbFNAM,
    wbStruct(SPDT, 'Data', [
      wbInteger('Type', itU32,
        wbEnum([
          {0} 'Spell',
          {1} 'Ability',
          {2} 'Blight',
          {3} 'Disease',
          {4} 'Curse',
          {5} 'Power'
        ])),
      wbInteger('Spell Cost', itU32),
      wbInteger('Flags', itU32,
        wbFlags([
          {0} 'Auto Calculate Cost',
          {1} 'Player Start Spell',
          {2} 'Always Succeeds'
        ])).IncludeFlag(dfCollapsed, wbCollapseFlags)
    ]).SetRequired,
    wbEffects
  ]).SetFormIDBase($0A);

  wbRecord(SSCR, 'Start Script', @wbKnownSubRecordSignaturesSSCR, [
    wbDELE,
    wbString(DATA, 'Numerical ID').SetRequired,
    wbString(NAME, 'Script').SetRequired //[SCPT]
  ]).SetFormIDBase($3F).SetAfterLoad(wbDELEAfterLoad);

  wbRecord(STAT, 'Static', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired
  ]).SetFormIDBase($40).SetSummaryKey([2]);

  wbRecord(WEAP, 'Weapon', wbFlags(wbFlagsList([
      10, 'References Persist',
      13, 'Blocked'
    ])), [
    wbNAME,
    wbDELE,
    wbMODL.SetRequired,
    wbFNAM,
    wbStruct(WPDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1),
      wbInteger('Type', itU16,
        wbEnum([
           {0} 'Short Blade One Hand',
           {1} 'Long Blade One Hand',
           {2} 'Long Blade Two Close',
           {3} 'Blunt One Hand',
           {4} 'Blunt Two Close',
           {5} 'Blunt Two Wide',
           {6} 'Spear Two Wide',
           {7} 'Axe One Hand',
           {8} 'Axe Two Hand',
           {9} 'Marksman Bow',
          {10} 'Marksman Crossbow',
          {11} 'Marksman Thrown',
          {12} 'Arrow',
          {13} 'Bolt'
        ])).SetDefaultNativeValue(12),
      wbInteger('Health', itU16).SetDefaultNativeValue(100),
      wbFloat('Speed', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbFloat('Reach', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Enchantment Charge', itU16).SetDefaultNativeValue(100),
      wbStruct('Damage Types', [
        wbStruct('Chop', [
          wbInteger('Minimum', itU8).SetDefaultNativeValue(1),
          wbInteger('Maximum', itU8).SetDefaultNativeValue(5)
        ]),
        wbStruct('Slash', [
          wbInteger('Minimum', itU8).SetDefaultNativeValue(1),
          wbInteger('Maximum', itU8).SetDefaultNativeValue(5)
        ]),
        wbStruct('Thrust', [
          wbInteger('Minimum', itU8).SetDefaultNativeValue(1),
          wbInteger('Maximum', itU8).SetDefaultNativeValue(5)
        ])
      ]),
      wbInteger('Flags', itU32,
        wbFlags([
          {0} 'Silver Weapon',
          {1} 'Ignore Normal Weapon Resistance'
        ])).IncludeFlag(dfCollapsed, wbCollapseFlags)
    ]).SetRequired,
    wbSCRI,
    wbITEX,
    wbENAM
  ]).SetFormIDBase($40);

  wbAddGroupOrder(GMST);
  wbAddGroupOrder(GLOB);
  wbAddGroupOrder(CLAS);
  wbAddGroupOrder(FACT);
  wbAddGroupOrder(RACE);
  wbAddGroupOrder(SOUN);
  wbAddGroupOrder(SKIL);
  wbAddGroupOrder(MGEF);
  wbAddGroupOrder(SCPT);
  wbAddGroupOrder(REGN);
  wbAddGroupOrder(SSCR);
  wbAddGroupOrder(BSGN);
  wbAddGroupOrder(LTEX);
  wbAddGroupOrder(STAT);
  wbAddGroupOrder(DOOR);
  wbAddGroupOrder(MISC);
  wbAddGroupOrder(WEAP);
  wbAddGroupOrder(CONT);
  wbAddGroupOrder(SPEL);
  wbAddGroupOrder(CREA);
  wbAddGroupOrder(BODY);
  wbAddGroupOrder(LIGH);
  wbAddGroupOrder(ENCH);
  wbAddGroupOrder(NPC_);
  wbAddGroupOrder(ARMO);
  wbAddGroupOrder(CLOT);
  wbAddGroupOrder(REPA);
  wbAddGroupOrder(ACTI);
  wbAddGroupOrder(APPA);
  wbAddGroupOrder(LOCK);
  wbAddGroupOrder(PROB);
  wbAddGroupOrder(INGR);
  wbAddGroupOrder(BOOK);
  wbAddGroupOrder(ALCH);
  wbAddGroupOrder(LEVI);
  wbAddGroupOrder(LEVC);
  wbAddGroupOrder(CELL);
  wbAddGroupOrder(LAND);
  wbAddGroupOrder(PGRD);
  wbAddGroupOrder(SNDG);
  wbAddGroupOrder(DIAL);
  wbAddGroupOrder(INFO);
  wbNexusModsUrl := 'https://www.nexusmods.com/morrowind/mods/54508';
  wbHEDRVersion := 1.30;
end;
end.
