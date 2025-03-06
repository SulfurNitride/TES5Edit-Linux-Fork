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
  wbBipedObjectEnum,
  wbDialogTypeEnum,
  wbMagicEffectEnum,
  wbSkillEnum,
  wbSpecializationEnum: IwbEnumDef;

  wbLeveledFlags: IwbFlagsDef;

  wbAIData,
  wbAIPackages,
  wbBipedObjects,
  wbDeleted,
  wbDescription,
  wbEditorID,
  wbEffects,
  wbEnchantment,
  wbFullName,
  wbIcon,
  wbModel,
  wbScript,
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
    '____',
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
    '____',
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

function wbEffectAreaDontShow(const aElement: IwbElement): Boolean;
begin
  Result := False;
  case Integer(aElement.Container.ElementNativeValues['Range']) of
    1,2: Result := True;
  end;
  if aElement.ContainingMainRecord.Signature = ALCH then
    Result := True
end;

function wbEffectAttributeDontShow(const aElement: IwbElement): Boolean;
begin
  Result := True;
  case Integer(aElement.Container.ElementNativeValues['Magic Effect']) of
    17,22,74,79,85: Result := False;
  end;
end;

function wbEffectDurationDontShow(const aElement: IwbElement): Boolean;
begin
  Result := False;
  case Integer(aElement.Container.ElementNativeValues['Magic Effect']) of
    12,13,57,60,61,62,63,69,70,71,72,73,133: Result := True;
  end;
end;

function wbEffectSkillDontShow(const aElement: IwbElement): Boolean;
begin
  Result := True;
  case Integer(aElement.Container.ElementNativeValues['Magic Effect']) of
    21,26,78,83,89: Result := False;
  end;
end;

procedure wbEffectRangeAfterLoad(const aElement: IwbElement);
var
  Container: IwbContainer;
begin
  if wbBeginInternalEdit then try
    if not Supports (aElement, IwbContainer, Container) then
      Exit;

    if Container.ElementEditValues['Range'] = '0' then
      Container.ElementNativeValues['Range'] := 1;

  finally
    wbEndInternalEdit
  end;
end;

procedure wbEffectRangeAfterSet(const aElement: IwbElement; const aOldValue, aNewValue: Variant);
begin
  aElement.Container.ElementByName['Range'].SetToDefault;
end;

function wbConditionFunctionDecider(aBasePtr: Pointer; aEndPtr: Pointer; const aElement: IwbElement): Integer;
var
  Container : IwbContainer;
begin
  Result := 0;
  if not wbTryGetContainerFromUnion(aElement, Container) then
    Exit;

  case Integer(Container.ElementNativeValues['Type']) of
    50,51,67: Result := 1;
    52: Result := 2;
    53: Result := 3;
    54: Result := 4;
    55: Result := 5;
    56: Result := 6;
    57: Result := 7;
    65: Result := 8;
    66: Result := 9;
  end;
end;

function wbEffectRangeDecider(aBasePtr: Pointer; aEndPtr: Pointer; const aElement: IwbElement): Integer;
var
  Container  : IwbContainer;
begin
  Result := 0;
  if not wbTryGetContainerFromUnion(aElement, Container) then
    Exit;

  case Integer(Container.ElementNativeValues['Magic Effect']) of
    12,13,44,49,50,51,52,53,54,55,56,58,85,86,87,88,89,101,118,119,126: Result :=1;
  end;
end;

function wbEffectRangeDontShow(const aElement: IwbElement): Boolean;
begin
  Result := False;
  case Integer(aElement.Container.ElementNativeValues['Magic Effect']) of
    59,60,61,62,63,64,65,66,102,103,104,105,106,107,108,109,110,111,112,113,
    114,115,116,120,121,122,123,124,125,127,128,129,130,131,132,133,134,135,
    137,138,139,140,141,142: Result := True;
  end;
  if aElement.ContainingMainRecord.Signature = ALCH then
    Result := True;
end;

function wbEffectMagnitudeDontShow(const aElement: IwbElement): Boolean;
begin
  Result := False;
  case Integer(aElement.Container.ElementNativeValues['Magic Effect']) of
    0,2,39,45,46,58,60,61,62,63,69,70,71,72,73,102,103,104,105,106,107,108,
    109,110,111,112,113,114,115,116,120,121,122,123,124,125,126,127,128,129,
    130,131,132,133,134,136,137,138,139,140,141,142: Result := True;
  end;
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

function GridCellToFormID(aFormIDBase: Byte; const aGridCell: TwbGridCell; out aFormID: TwbFormID): Boolean;
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
var
  SubRecord: IwbSubRecord;
begin
  Result := 0;
  if Assigned(aElement) and Supports(aElement.Container, IwbSubRecord, SubRecord) then
    if SubRecord.SubRecordHeaderSize = 12 then
      Result := 1;
end;

procedure DefineTES3;
begin
  DefineCommon;
  wbHeaderSignature := 'TES3';

  wbRecordFlags :=
    wbInteger('Record Flags', itU32,
      wbFlags(wbSparseFlags([
      0,  'ESM',
      5,  'Deleted',
      10, 'References Persist',
      13, 'Blocked'
      ], False, 14)));

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

  wbBipedObjectEnum :=
    wbEnum ([
    {0}  'Head',
    {1}  'Hair',
    {2}  'Neck',
    {3}  'Chest',
    {4}  'Groin',
    {5}  'Skirt',
    {6}  'Right Hand',
    {7}  'Left Hand',
    {8}  'Right Wrist',
    {9}  'Left Wrist',
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
    {0}   'Water Breathing',
    {1}   'Swift Swim',
    {2}   'Water Walking',
    {3}   'Shield',
    {4}   'Fire Shield',
    {5}   'Lightning Shield',
    {6}   'Frost Shield',
    {7}   'Burden',
    {8}   'Feather',
    {9}   'Jump',
    {10}  'Levitate',
    {11}  'Slow Fall',
    {12}  'Lock',
    {13}  'Open',
    {14}  'Fire Damage',
    {15}  'Shock Damage',
    {16}  'Frost Damage',
    {17}  'Drain Attribute',
    {18}  'Drain Health',
    {19}  'Drain Magicka',
    {20}  'Drain Fatigue',
    {21}  'Drain Skill',
    {22}  'Damage Attribute',
    {23}  'Damage Health',
    {24}  'Damage Magicka',
    {25}  'Damage Fatigue',
    {26}  'Damage Skill',
    {27}  'Poison',
    {28}  'Weakness To Fire',
    {29}  'Weakness To Frost',
    {30}  'Weakness To Shock',
    {31}  'Weakness To Magicka',
    {32}  'Weakness To Common Disease',
    {33}  'Weakness To Blight Disease',
    {34}  'Weakness To Corprus Disease',
    {35}  'Weakness To Poison',
    {36}  'Weakness To Normal Weapons',
    {37}  'Disintegrate Weapon',
    {38}  'Disintegrate Armor',
    {39}  'Invisibility',
    {40}  'Chameleon',
    {41}  'Light',
    {42}  'Sanctuary',
    {43}  'Night Eye',
    {44}  'Charm',
    {45}  'Paralyze',
    {46}  'Silence',
    {47}  'Blind',
    {48}  'Sound',
    {49}  'Calm Humanoid',
    {50}  'Calm Creature',
    {51}  'Frenzy Humanoid',
    {52}  'Frenzy Creature',
    {53}  'Demoralize Humanoid',
    {54}  'Demoralize Creature',
    {55}  'Rally Humanoid',
    {56}  'Rally Creature',
    {57}  'Dispel',
    {58}  'Soultrap',
    {59}  'Telekinesis',
    {60}  'Mark',
    {61}  'Recall',
    {62}  'Divine Intervention',
    {63}  'Almsivi Intervention',
    {64}  'Detect Animal',
    {65}  'Detect Enchantment',
    {66}  'Detect Key',
    {67}  'Spell Absorption',
    {68}  'Reflect',
    {69}  'Cure Common Disease',
    {70}  'Cure Blight Disease',
    {71}  'Cure Corprus Disease',
    {72}  'Cure Poison',
    {73}  'Cure Paralyzation',
    {74}  'Restore Attribute',
    {75}  'Restore Health',
    {76}  'Restore Magicka',
    {77}  'Restore Fatigue',
    {78}  'Restore Skill',
    {79}  'Fortify Attribute',
    {80}  'Fortify Health',
    {81}  'Fortify Magicka',
    {82}  'Fortify Fatigue',
    {83}  'Fortify Skill',
    {84}  'Fortify Maximum Magicka',
    {85}  'Absorb Attribute',
    {86}  'Absorb Health',
    {87}  'Absorb Magicka',
    {88}  'Absorb Fatigue',
    {89}  'Absorb Skill',
    {90}  'Resist Fire',
    {91}  'Resist Frost',
    {92}  'Resist Shock',
    {93}  'Resist Magicka',
    {94}  'Resist Common Disease',
    {95}  'Resist Blight Disease',
    {96}  'Resist Corprus Disease',
    {97}  'Resist Poison',
    {98}  'Resist Normal Weapons',
    {99}  'Resist Paralysis',
    {100} 'Remove Curse',
    {101} 'Turn Undead',
    {102} 'Summon Scamp',
    {103} 'Summon Clannfear',
    {104} 'Summon Daedroth',
    {105} 'Summon Dremora',
    {106} 'Summon Ancestral Ghost',
    {107} 'Summon Skeletal Minion',
    {108} 'Summon Bonewalker',
    {109} 'Summon Greater Bonewalker',
    {110} 'Summon Bonelord',
    {111} 'Summon Winged Twilight',
    {112} 'Summon Hunger',
    {113} 'Summon Golden Saint',
    {114} 'Summon Flame Atronach',
    {115} 'Summon Frost Atronach',
    {116} 'Summon Storm Atronach',
    {117} 'Fortify Attack',
    {118} 'Command Creature',
    {119} 'Command Humanoid',
    {120} 'Bound Dagger',
    {121} 'Bound Longsword',
    {122} 'Bound Mace',
    {123} 'Bound Battle Axe',
    {124} 'Bound Spear',
    {125} 'Bound Longbow',
    {126} 'EXTRA SPELL',
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
    {138} 'sEffectSummonCreature01',
    {139} 'sEffectSummonCreature02',
    {140} 'sEffectSummonCreature03',
    {141} 'sEffectSummonCreature04',
    {142} 'sEffectSummonCreature05'
    ], [
    -1, 'None'
    ]);

  wbSkillEnum :=
    wbEnum([
    {0}  'Block',
    {1}  'Armorer',
    {2}  'Medium Armor',
    {3}  'Heavy Armor',
    {4}  'Blunt Weapon',
    {5}  'Long Blade',
    {6}  'Axe',
    {7}  'Spear',
    {8}  'Athletics',
    {9}  'Enchant',
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
    {26} 'Hand-To-Hand'
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

  wbDeleted := wbInteger(DELE, 'Deleted', itU32, wbEnum(['Deleted']));
  wbDescription := wbString(DESC, 'Description');
  wbEditorID := wbString(NAME, 'Editor ID');
  wbEnchantment := wbString(ENAM, 'Enchantment');
  wbFullName := wbString(FNAM, 'Name');
  wbIcon := wbString(ITEX, 'Icon Filename');
  wbModel := wbString(MODL, 'Model Filename').SetDefaultEditValue('Add Art File');
  wbScript := wbString(SCRI, 'Script');
  {>>> Record Members <<<}

  wbAIData :=
    wbStruct(AIDT, 'AI Data', [
      wbInteger('Hello', itU16),
      wbInteger('Fight', itU8),
      wbInteger('Flee', itU8),
      wbInteger('Alarm', itU8),
      wbUnused(3),
      wbInteger('Service Flags', itU32, wbServiceFlags)
    ]).SetRequired;

  wbAIPackages :=
    wbRArray('AI Packages',
      wbRUnion('AI Packages', [
        wbStruct(AI_T, 'AI Travel', [
          wbVec3('Position'),
          wbInteger('Reset', itU8, wbBoolEnum),
          wbUnused(3)
        ]).SetRequired,
        wbStruct(AI_W, 'AI Wander', [
          wbInteger('Distance', itU16),
          wbInteger('Duration In Hours', itU16),
          wbInteger('Time of Day', itU8),
          wbStruct('Idle Chances', [
            wbInteger('Idle 2', itU8),
            wbInteger('Idle 3', itU8),
            wbInteger('Idle 4', itU8),
            wbInteger('Idle 5', itU8),
            wbInteger('Idle 6', itU8),
            wbInteger('Idle 7', itU8),
            wbInteger('Idle 8', itU8),
            wbInteger('Idle 9', itU8)
          ]),
          wbInteger('Reset', itU8, wbBoolEnum)
        ]).SetRequired,
        wbRStruct('AI Escort', [
          wbStruct(AI_E, 'AI Escort', [
            wbVec3('Position'),
            wbInteger('Duration In Hours', itU16),
            wbString(True, 'Target', 32), //[CREA, NPC_]
            wbInteger('Reset', itU16, wbBoolEnum)
          ]).SetRequired,
          wbString(CNDT, 'Escort To Cell') //[CELL]
        ]),
        wbRStruct('AI Follow', [
          wbStruct(AI_F, 'AI Follow', [
            wbVec3('Position'),
            wbInteger('Duration In Hours', itU16),
            wbString(True, 'Target', 32), //[CREA, NPC_]
            wbInteger('Reset', itU16, wbBoolEnum)
          ]).SetRequired,
          wbString(CNDT, 'Follow To Cell') //[CELL]
        ]),
        wbStruct(AI_A, 'AI Activate', [
          wbString(True, 'Target', 32), //[ACTI, ALCH, APPA, ARMO, BODY, BOOK, CLOT, CONT, CREA, DOOR, ENCH, INGR, LIGH, LEVC, LEVI, LOCK, MISC, NPC_, PROB, REPA, SPEL, STAT, WEAP]
          wbInteger('Reset', itU8, wbBoolEnum)
        ]).SetRequired
      ]));

  wbBipedObjects :=
    wbRArray('Biped Objects',
      wbRStruct('Biped Object', [
        wbInteger(INDX, 'Body Part', itU8, wbBipedObjectEnum),
        wbString(BNAM, 'Male Armor'), //[BODY]
        wbString(CNAM, 'Female Armor') //[BODY]
      ]).SetRequired);

  wbEffects :=
    wbRArray('Effects',
      wbStruct(ENAM, 'Effect', [
        wbInteger('Magic Effect', itS16, wbMagicEffectEnum)
          .SetAfterSet(wbEffectRangeAfterSet)
          .SetDefaultNativeValue(-1),
        wbInteger('Skill', itS8, wbSkillEnum)
          .SetDefaultNativeValue(-1)
          .SetDontShow(wbEffectSkillDontShow),
        wbInteger('Attribute', itS8, wbAttributeEnum)
          .SetDefaultNativeValue(-1)
          .SetDontShow(wbEffectAttributeDontShow),
        wbUnion('Range', wbEffectRangeDecider, [
          wbInteger('Range', itU32,
            wbEnum([
            {0} 'Self',
            {1} 'Touch',
            {2} 'Target'
            ])),
          wbInteger('Range', itU32,
            wbEnum([], [
            1, 'Touch',
            2, 'Target'
            ])).SetDefaultNativeValue(1)
          ]).SetDontShow(wbEffectRangeDontShow),
        wbInteger('Area', itU32).SetDontShow(wbEffectAreaDontShow),
        wbInteger('Duration', itU32).SetDontShow(wbEffectDurationDontShow),
        wbInteger('Magnitude Minimum', itU32).SetDontShow(wbEffectMagnitudeDontShow),
        wbInteger('Magnitude Maximum', itU32).SetDontShow(wbEffectMagnitudeDontShow)
      ]).SetAfterLoad(wbEffectRangeAfterLoad));

  wbTravelServices :=
    wbRArray('Travel Services',
      wbRStruct('Travel Service', [
        wbStruct(DODT, 'Destination', [
          wbVec3('Position'),
          wbVec3('Rotation')
        ]).SetRequired,
        wbStringForward(DNAM, 'Cell', 64)
      ]));

  {>>> Records <<<}

  wbRecord(TES3, 'Main File Header', [
    wbStruct(HEDR, 'Header', [
      wbFloat('Version'),
      wbRecordFlags,
      wbString('Author', 32),
      wbString('Description', 256),
      wbInteger('Number of Records', itU32)
    ]).SetRequired,
    wbRArray('Master Files',
      wbRStruct('Master File', [
        wbStringForward(MAST, 'Filename').SetRequired,
        wbInteger(DATA, 'Master Size', itU64, nil, cpIgnore, True)
    ])).IncludeFlag(dfInternalEditOnly, not wbAllowMasterFilesEdit)
  ], False, nil, cpNormal, True)
    .SetGetFormIDCallback(function(const aMainRecord: IwbMainRecord; out aFormID: TwbFormID): Boolean begin
       Result := True;
       aFormID := TwbFormID.Null;
     end);

  wbRecord(ACTI, 'Activator', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbScript //[SCPT]
  ]).SetFormIDBase($40);

  wbRecord(ALCH, 'Alchemy', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbString(TEXT, 'Icon Filename'),
    wbScript, //[SCPT]
    wbFullName,
    wbStruct(ALDT, 'Alchemy Data', [
      wbFloat('Weight', cpNormal, False, 1, 2),
      wbInteger('Value', itS32),
      wbInteger('Auto Calculate Value', itU32, wbBoolEnum)
    ]).SetRequired,
    wbEffects
  ]).SetFormIDBase($40);

  wbRecord(APPA, 'Apparatus', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbScript, //[SCPT]
    wbStruct(AADT, 'Data', [
      wbInteger('Type', itU32,
        wbEnum([
        {0} 'Mortar & Pestle',
        {1} 'Alembic',
        {2} 'Calcinator',
        {3} 'Retort'
        ])),
      wbFloat('Quality', cpNormal, False, 1, 2),
      wbFloat('Weight', cpNormal, False, 1, 2),
      wbInteger('Value', itS32)
    ]).SetRequired,
    wbIcon
  ]).SetFormIDBase($40);

  wbRecord(ARMO, 'Armor', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbScript, //[SCPT]
    wbStruct(AODT, 'Data', [
      wbInteger('Type', itU32,
        wbEnum([
        {0}  'Helmet',
        {1}  'Cuirass',
        {2}  'Left Pauldron',
        {3}  'Right Pauldron',
        {4}  'Greaves',
        {5}  'Boots',
        {6}  'Left Gauntlet',
        {7}  'Right Gauntlet',
        {8}  'Shield',
        {9}  'Left Bracer',
        {10} 'Right Bracer'
        ])),
      wbFloat('Weight', cpNormal, False, 1, 2),
      wbInteger('Value', itS32),
      wbInteger('Health', itS32),
      wbInteger('Enchanting Charge', itS32),
      wbInteger('Armor Rating', itS32)
    ]).SetRequired,
    wbIcon,
    wbBipedObjects,
    wbEnchantment //[ENCH]
  ]).SetFormIDBase($40);

  wbRecord(BODY, 'Body Part', @wbKnownSubRecordSignaturesNoFNAM, [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbString(FNAM, 'Skin Race'), //[RACE]
    wbStruct(BYDT, 'Data', [
      wbInteger('Part', itU8,
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
        ])),
      wbInteger('Skin Type', itU8,
        wbEnum([
        {0} 'Normal',
        {1} 'Vampire'
        ])),
      wbInteger('Flags', itU8,
        wbFlags([
        {0} 'Female',
        {1} 'Not Playable'
        ])),
      wbInteger('Part Type', itU8,
        wbEnum([
        {0} 'Skin',
        {1} 'Clothing',
        {2} 'Armor'
        ]))
    ]).SetRequired
  ]).SetFormIDBase($20);

  wbRecord(BOOK, 'Book', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbStruct(BKDT, 'Book Data', [
      wbFloat('Weight', cpNormal, False, 1, 2),
      wbInteger('Value', itS32),
      wbInteger('Is Scroll', itU32, wbBoolEnum),
      wbInteger('Teaches', itS32, wbSkillEnum), //[SKIL]
      wbInteger('Enchanting Charge', itS32)
    ]).SetRequired,
    wbScript, //[SCPT]
    wbIcon,
    wbLStringKC(TEXT, 'Book Text'),
    wbEnchantment //[ENCH]
  ]).SetFormIDBase($40);

  wbRecord(BSGN, 'Birthsign', [
    wbDeleted,
    wbEditorID,
    wbFullName,
    wbString(TNAM, 'Constellation Image'),
    wbDescription,
    wbRArray('Spells', wbStringForward(NPCS, 'Spell', 32)) //[SPEL]
  ]).SetFormIDBase($10);

  wbRecord(CELL, 'Cell', [
    wbEditorID,
    wbDeleted,
    wbStruct(DATA, 'Data', [
      wbInteger('Flags', itU32,
        wbFlags(wbSparseFlags([
        0, 'Is Interior Cell',
        1, 'Has Water',
        2, 'Illegal to Sleep Here',
        6, 'Has Map Color',
        7, 'Behave Like Exterior'
        ], False, 8))),
      wbStruct('Grid', [
        wbInteger('X', itS32),
        wbInteger('Y', itS32)
      ])
    ]).SetRequired,
    wbInteger(INTV, 'Water Height (Old Format)', itS32),
    wbString(RGNN, 'Region'),  //[REGN]
    wbByteColors(NAM5, 'Map Color'),
    wbFloat(WHGT, 'Water Height'),
    wbStruct(AMBI, 'Ambience', [
      wbByteColors('Ambient Color'),
      wbByteColors('Sunlight Color'),
      wbByteColors('Fog Color'),
      wbFloat('Fog Density', cpNormal, False, 1, 2)
    ])
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
      Result := aMainRecord.GetGridCell(GridCell) and GridCellToFormID($A0, GridCell, aFormID);
    end)
    .SetIdentityCallback(function(const aMainRecord: IwbMainRecord): string begin
      var GridCell: TwbGridCell;
      if aMainRecord.GetGridCell(GridCell) then
        Result := '<Exterior>' + GridCell.SortKey
      else
        Result := aMainRecord.EditorID;
    end);

  wbRecord(CLAS, 'Class', [
    wbEditorID,
    wbDeleted,
    wbFullName,
    wbStruct(CLDT, 'Data', [
      wbArray('Primary Attributes',
        wbInteger('Primary Attribute', itS32, wbAttributeEnum),
      2),
      wbInteger('Specialization', itU32, wbSpecializationEnum),
      wbArray('Major & Minor Skill Sets',
        wbStruct('Skill Set', [
          wbInteger('Minor', itS32, wbSkillEnum), //[SKIL]
          wbInteger('Major', itS32, wbSkillEnum) //[SKIL]
        ]),
      5),
      wbInteger('Playable', itU32, wbBoolEnum),
      wbInteger('Service Flags', itU32, wbServiceFlags)
    ]).SetRequired,
    wbDescription
  ]).SetFormIDBase($18);

  wbRecord(CLOT, 'Clothing', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
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
      ])),
      wbFloat('Weight', cpNormal, False, 1.0, 2),
      wbInteger('Value', itU16),
      wbInteger('Enchanting Charge', itU16)
    ]).SetRequired,
    wbScript, //[SCPT]
    wbIcon,
    wbBipedObjects,
    wbEnchantment //[ENCH]
  ]).SetFormIDBase($40);

  wbRecord(CONT, 'Container', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbFloat(CNDT, 'Weight', cpNormal, False, 1.0, 2),
    wbInteger(FLAG, 'Flags', itU32,
      wbFlags(wbSparseFlags([
      0, 'Organic',
      1, 'Respawns',
      3, 'Can Hold Items'
      ], False, 4))),
    wbScript, //[SCPT]
    wbRArray('Item Entries',
      wbStruct(NPCO, 'Item Entry', [
        wbInteger('Count', itS32),
        wbString('Item', 32) //[ALCH, APPA, ARMO, BOOK, CLOT, INGR, LEVI, LIGH, LOCK, MISC, PROB, REPA, WEAP]
      ]))
  ]).SetFormIDBase($40);

  wbRecord(CREA, 'Creature', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbString(CNAM, 'Sound Generator Creature'), //[CREA]
    wbFullName,
    wbScript, //[SCPT]
    wbStruct(NPDT, 'Data', [
      wbInteger('Type', itU32,
        wbEnum([
        {0} 'Creature',
        {1} 'Daedra',
        {2} 'Undead',
        {3} 'Humanoid'
        ])),
      wbInteger('Level', itS32),
      wbStruct('Attributes', [
        wbInteger('Strength', itS32),
        wbInteger('Intelligence', itS32),
        wbInteger('Willpower', itS32),
        wbInteger('Agility', itS32),
        wbInteger('Speed', itS32),
        wbInteger('Endurance', itS32),
        wbInteger('Personality', itS32),
        wbInteger('Luck', itS32)
      ]),
      wbInteger('Health', itS32),
      wbInteger('Magicka', itS32),
      wbInteger('Fatigue', itS32),
      wbInteger('Soul', itU32),
      wbStruct('Skills', [
        wbInteger('Combat', itS32),
        wbInteger('Magic', itS32),
        wbInteger('Stealth', itS32)
      ]),
      wbArray('Attack Sets',
        wbStruct('Attack Set', [
          wbInteger('Minimum', itS32),
          wbInteger('Maximum', itS32)
        ]),
      3),
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
      ], False, 12))
    ).IncludeFlag(dfCollapsed, wbCollapseFlags),
    wbFloat(XSCL, 'Scale', cpNormal, False, 1.0, 2),
    wbRArray('Item Entries',
      wbStruct(NPCO, 'Item Entry', [
        wbInteger('Count', itS32),
        wbString('Item', 32) //[ALCH, APPA, ARMO, BOOK, CLOT, INGR, LEVI, LIGH, LOCK, MISC, PROB, REPA, WEAP]
      ])),
    wbRArray('Spells', wbString(NPCS, 'Spell', 32)), //[SPEL]
    wbAIData,
    wbTravelServices,
    wbAIPackages
  ]).SetFormIDBase($40);

  wbRecord(DIAL, 'Dialog Topic', [
    wbEditorID,
    wbStruct(DATA, 'Data', [
      wbInteger('Dialog Type', itU8, wbDialogTypeEnum),
      wbUnused(3)
    ]),
    wbDeleted
  ]).SetFormIDBase($80);

  wbRecord(DOOR, 'Door', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbScript, //[SCPT]
    wbString(SNAM, 'Open Sound'), //[SOUN]
    wbString(ANAM, 'Close Sound') //[SOUN]
  ]).SetFormIDBase($40);

  wbRecord(ENCH, 'Enchantment', [
    wbEditorID,
    wbDeleted,
    wbStruct(ENDT, 'Data', [
      wbInteger('Cast Type', itU32,
        wbEnum([
        {0} 'Cast Once',
        {1} 'Cast Strikes',
        {2} 'Cast When Used',
        {3} 'Constant Effect'
        ])),
      wbInteger('Enchantment Cost', itS32),
      wbInteger('Charge Amount', itS32),
      wbInteger('Flags', itU8,
        wbFlags([
        {0} 'Auto Calculate'
        ], True)),
      wbUnused(3)
    ]).SetRequired,
    wbEffects
  ]).SetFormIDBase($04);

  wbRecord(FACT, 'Faction', [
    wbEditorID,
    wbDeleted,
    wbFullName,
    wbRArray('Ranks', wbStringForward(RNAM, 'Rank', 32)),
    wbStruct(FADT, 'Data', [
      wbArray('Attributes',
        wbInteger('Attribute', itS32, wbAttributeEnum),
      2),
      wbArray('Ranks',
        wbStruct('Rank', [
          wbArray('Attribute Values',
            wbInteger('Attribute Value', itU32),
          2),
          wbInteger('Primary Skills Value', itU32),
          wbInteger('Favored Skills Value', itU32),
          wbInteger('Faction Reputation', itU32)
        ]),
      10),
      wbArray('Favored Skills',
        wbInteger('Skill', itS32, wbSkillEnum),
      7),
      wbInteger('Hidden From Player', itU32, wbBoolEnum)
    ]).SetRequired,
    wbRArray('Relations',
      wbRStructSK([], 'Relation', [
        wbString(ANAM, 'Faction'), //[FACT]
        wbInteger(INTV, 'Reaction', itS32)
      ]))
  ]).SetFormIDBase($1C);

  wbRecord(GLOB, 'Global', @wbKnownSubRecordSignaturesNoFNAM,  [
    wbEditorID,
    wbDeleted,
    wbInteger(FNAM, 'Variable Type', itU8,
      wbEnum([], [
      $66, 'Float',
      $6C, 'Long',
      $73, 'Short'
      ])),
    wbFloat(FLTV, 'Value')
  ]).SetFormIDBase($58);

  wbRecord(GMST, 'Game Setting', [
    wbEditorID,
    wbRUnion('Value', [
      wbString(STRV, 'Value - String'),
      wbInteger(INTV, 'Value - Signed Integer', itS32),
      wbFloat(FLTV, 'Value - Float')
    ])
  ]).SetFormIDBase($50)
    .IncludeFlag(dfIndexEditorID);

  wbRecord(INFO, 'Dialog Response', @wbKnownSubRecordSignaturesINFO, [
    wbString(INAM, 'Response ID'),
    wbString(PNAM, 'Previous Response ID'),
    wbString(NNAM, 'Next Response ID'),
    wbStruct(DATA, 'Data', [
      wbInteger('Dialog Type', itU32, wbDialogTypeEnum),
      wbInteger('Disposition/Index', itU32),
      wbInteger('Speaker Faction Rank', itS8),
      wbInteger('Gender', itS8, wbSexEnum),
      wbInteger('Player Faction Rank', itS8),
      wbUnused(1)
    ]).SetRequired,
    wbString(ONAM, 'Speaker'), //[NPC_]
    wbString(RNAM, 'Speaker Race'), //[RACE]
    wbString(CNAM, 'Speaker Class'), //[CLAS]
    wbString(FNAM, 'Speaker In Faction'), //[FACT]
    wbString(ANAM, 'Speaker In Cell'), //[CELL]
    wbString(DNAM, 'Player Faction'), //[FACT]
    wbString(SNAM, 'Sound Filename'),
    wbString(NAME, 'Response'),
    wbDeleted,
    wbRStruct('Quest Data', [
      wbInteger(QSTN, 'Quest Name', itU8,
        wbEnum([], [1, 'Quest Name'])
      ).SetDefaultNativeValue(1),
      wbInteger(QSTF, 'Quest Finished', itU8,
        wbEnum([], [1, 'Quest Finished'])
      ).SetDefaultNativeValue(1),
      wbInteger(QSTR, 'Quest Restarted', itU8,
        wbEnum([], [1, 'Quest Restarted'])
      ).SetDefaultNativeValue(1)
    ], [], cpNormal, False, nil, True),
    wbRArray('Conditions',
      wbRStructSK([], 'Condition', [
        wbStruct(SCVR, 'Condition Breakdown', [
          wbInteger('Position', itU8,
            wbEnum([], [
            48, '1st', //0
            49, '2nd', //1
            50, '3rd', //2
            51, '4th', //3
            52, '5th', //4
            53, '6th' //5
            ])),
          wbInteger('Type', itU8,
            wbEnum([], [
            49, 'Function',
            50, 'Global',
            51, 'Local',
            52, 'Journal',
            53, 'Item',
            54, 'Dead',
            55, 'Not ID',
            56, 'Not Faction',
            57, 'Not Class',
            65, 'Not Race',
            66, 'Not Cell',
            67, 'Not Local'
            ])),
          wbUnion('Function', wbConditionFunctionDecider, [
            wbInteger('Function', itU16,
              wbEnum([], [
              12336, 'Reaction Low',
              12337, 'PC Strength',
              12338, 'PC Enchant',
              12339, 'PC Sneak',
              12340, 'PC Common Disease',
              12341, 'Choice',
              12342, 'PC Vampire',
              12343, 'Flee',
              12592, 'Reaction High',
              12593, 'PC Block',
              12594, 'PC Destruction',
              12595, 'PC Acrobatics',
              12596, 'PC Blight Disease',
              12597, 'PC Intelligence',
              12598, 'Level',
              12599, 'Should Attack',
              12848, 'Rank Requirement',
              12849, 'PC Armorer',
              12850, 'PC Alteration',
              12851, 'PC Light Armor',
              12852, 'PC Clothing Modifier',
              12853, 'PC Willpower',
              12854, 'Attacked',
              12855, 'Werewolf',
              13104, 'Reputation',
              13105, 'PC Medium Armor',
              13106, 'PC Illusion',
              13107, 'PC Short Blade',
              13108, 'PC Crime Level',
              13109, 'PC Agility',
              13110, 'Talked To PC',
              13111, 'PC Werewolf Kills',
              13360, 'Health Percent',
              13361, 'PC Heavy Armor',
              13362, 'PC Conjuration',
              13363, 'PC Marksman',
              13364, 'Same Sex',
              13365, 'PC Speed',
              13366, 'PC Health',
              13616, 'PC Reputation',
              13617, 'PC Blunt Weapon',
              13619, 'PC Mysticism',
              13619, 'PC Mercantile',
              13620, 'Same Race',
              13621, 'PC Endurance',
              13622, 'Creature Target',
              13872, 'PC Level',
              13873, 'PC Long Blade',
              13874, 'PC Restoration',
              13875, 'PC Speechcraft',
              13876, 'Same Faction',
              13877, 'PC Personality',
              13878, 'Friend Hit',
              14128, 'PC Health Percent',
              14129, 'PC Axe',
              14130, 'PC Alchemy',
              14131, 'PC Hand To Hand',
              14132, 'Faction Rank Difference',
              14133, 'PC Luck',
              14134, 'Fight',
              14384, 'PC Magicka',
              14385, 'PC Spear',
              14386, 'PC Unarmored',
              14387, 'PC Sex',
              14388, 'Detected',
              14389, 'PC Corpus',
              14390, 'Hello',
              14640, 'PC Fatigue',
              14641, 'PC Athletics',
              16462, 'PC Security',
              14643, 'PC Expelled',
              14644, 'Alarmed',
              14645, 'Weather',
              14646, 'Alarm'
              ])).SetDefaultNativeValue(14646),
            wbInteger('Function', itU16,
              wbEnum([], [
              22630, 'Float in FLTV',
              22636, 'Long in INTV',
              22643, 'Short in INTV'
              ])).SetDefaultNativeValue(22630),
            wbInteger('Function', itU16,
              wbEnum([], [
              22602, 'Journal'
              ])).SetDefaultNativeValue(22602),
            wbInteger('Function', itU16,
              wbEnum([], [
              22601, 'Item'
              ])).SetDefaultNativeValue(22601),
            wbInteger('Function', itU16,
              wbEnum([], [
              22596, 'Dead'
              ])).SetDefaultNativeValue(22596),
            wbInteger('Function', itU16,
              wbEnum([], [
              22616, 'Not ID'
              ])).SetDefaultNativeValue(22616),
            wbInteger('Function', itU16,
              wbEnum([], [
              22598, 'Not Faction'
              ])).SetDefaultNativeValue(22598),
            wbInteger('Function', itU16,
              wbEnum([], [
              22595, 'Not Class'
              ])).SetDefaultNativeValue(22595),
            wbInteger('Function', itU16,
              wbEnum([], [
              22610, 'Not Race'
              ])).SetDefaultNativeValue(22610),
            wbInteger('Function', itU16,
              wbEnum([], [
              22604, 'Not Cell'
              ])).SetDefaultNativeValue(22604)
            ]),
          wbInteger('Operator', itU8,
            wbEnum([], [
            48, 'Equal To',
            49, 'Not Equal To',
            50, 'Less Than',
            51, 'Less Than or Equal To',
            52, 'Greater Than',
            53, 'Greater Than or Equal To'
            ])),
          wbString('Variable/Object')
        ]),
        wbRUnion('Value', [
          wbInteger(INTV, 'Value', itS32),
          wbFloat(FLTV, 'Value')
        ])
      ])),
    wbString(BNAM, 'Result')
  ]).SetFormIDBase($90);

  wbRecord(INGR, 'Ingredient', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbStruct(IRDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1.0, 2),
      wbInteger('Value', itS32),
      wbStruct('Effects', [
        wbArray('Magic Effects',
          wbInteger('Magic Effect', itS32, wbMagicEffectEnum),
        4),
        wbArray('Skills',
          wbInteger('Skill', itS32,
            wbEnum([
            {0}  'Block (None for Attribute based Magic Effects)',
            {1}  'Armorer',
            {2}  'Medium Armor',
            {3}  'Heavy Armor',
            {4}  'Blunt Weapon',
            {5}  'Long Blade',
            {6}  'Axe',
            {7}  'Spear',
            {8}  'Athletics',
            {9}  'Enchant',
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
            {26} 'Hand-To-Hand'
            ], [
            -1, 'None'
            ])),
        4),
        wbArray('Attributes',
          wbInteger('Attribute', itS32,
            wbEnum([
            {0} 'Strength (None for Skill based Magic Effects)',
            {1} 'Intelligence',
            {2} 'Willpower',
            {3} 'Agility',
            {4} 'Speed',
            {5} 'Endurance',
            {6} 'Personality',
            {7} 'Luck'
            ], [
            -1, 'None'
            ])),
        4)
      ])
    ]).SetRequired,
    wbScript, //[SCPT]
    wbIcon
  ]).SetFormIDBase($40);

  wbRecord(LAND, 'Landscape', @wbKnownSubRecordSignaturesLAND, [
    wbStruct(INTV, 'Grid', [
      wbInteger('X', itS32),
      wbInteger('Y', itS32)
    ], cpCritical).SetRequired,
    wbInteger(DATA, 'Flags', itU32,
      wbFlags([
      {0} 'Has Vertex Normals/Height Map',
      {1} 'Has Vertex Colors',
      {2} 'Has Landscape Textures',
      {3} 'User Created/Edited'
      ])),
    IfThen(wbSimpleRecords,
      wbByteArray(VNML, 'Vertex Normals'),
      wbArray(VNML, 'Vertex Normals',
        wbArray('Row',
          wbStruct('Column', [
            wbInteger('X', itS8),
            wbInteger('Y', itS8),
            wbInteger('Z', itS8)
          ]),
        65),
      65)),
    IfThen(wbSimpleRecords,
      wbByteArray(VHGT, 'Vertex Height Map'),
      wbStruct(VHGT, 'Vertex Height Map', [
        wbFloat('Offset'),
        wbUnused(1),
        wbArray('Height Map',
          wbArray('Row',
            wbInteger('Column', itS8),
          65),
        65),
        wbUnused(2)
      ])),
    IfThen(wbSimpleRecords,
      wbByteArray(WNAM, 'World Map Painting'),
      wbArray(WNAM, 'World Map Painting',
        wbArray('Row',
          wbInteger('Column', itS8),
        9),
      9)),
    IfThen(wbSimpleRecords,
      wbByteArray(VCLR, 'Vertex Colors'),
      wbArray(VCLR, 'Vertex Colors',
        wbArray('Row',
          wbStruct('Column', [
            wbInteger('Red', itU8),
            wbInteger('Green', itU8),
            wbInteger('Blue', itU8)
          ]).SetToStr(wbRGBAToStr)
            .IncludeFlag(dfCollapsed, wbCollapseRGBA),
        65),
      65)),
    IfThen(wbSimpleRecords,
      wbByteArray(VTXT, 'Textures'),
      wbArray(VTEX, 'Textures',
        wbArray('Row',
          wbInteger('Column', itU16), //[LTEX]
        16),
      16))
  ]).SetFormIDBase($D0)
    .SetFormIDNameBase($B0)
    .SetGetFormIDCallback(function(const aMainRecord: IwbMainRecord; out aFormID: TwbFormID): Boolean begin
      var GridCell: TwbGridCell;
      Result := aMainRecord.GetGridCell(GridCell) and GridCellToFormID($C0, GridCell, aFormID);
    end)
    .SetIdentityCallback(function(const aMainRecord: IwbMainRecord): string begin
      Result := '';
      var GridCell: TwbGridCell;
      if aMainRecord.GetGridCell(GridCell) then
        Result := GridCell.SortKey
    end);

  wbRecord(LEVC, 'Leveled Creature', [
    wbEditorID,
    wbDeleted,
    wbInteger(DATA, 'Leveled Flags', itU32, wbLeveledFlags),
    wbInteger(NNAM, 'Chance None', itU8),
    wbInteger(INDX, 'Entry Count', itU32).IncludeFlag(dfSkipImplicitEdit),
    wbRArray('Leveled Creature Entries',
      wbRStruct('Leveled Creature Entry', [
        wbString(CNAM, 'Creature'), //[CREA]
        wbInteger(INTV, 'Player Level', itU16)
      ])).SetCountPath(INDX)
  ]).SetFormIDBase($40);

  wbRecord(LEVI, 'Leveled Item', [
    wbEditorID,
    wbDeleted,
    wbInteger(DATA, 'Levelved Flags', itU32, wbLeveledFlags),
    wbInteger(NNAM, 'Chance None', itU8),
    wbInteger(INDX, 'Entry Count', itU32).IncludeFlag(dfSkipImplicitEdit),
    wbRArray('Leveled Item Entries',
      wbRStruct('Leveled Item Entry', [
        wbString(INAM, 'Item'), //[ALCH, APPA, ARMO, BOOK, CLOT, INGR, LEVI, LIGH, LOCK, MISC, PROB, REPA, WEAP]
        wbInteger(INTV, 'Player Level', itU16)
      ])).SetCountPath(INDX)
  ]).SetFormIDBase($40);

  wbRecord(LIGH, 'Light', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbIcon,
    wbStruct(LHDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1.0, 2),
      wbInteger('Value', itS32),
      wbInteger('Time', itS32),
      wbFloat('Radius', cpNormal, False, 1.0, 2),
      wbByteColors,
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
    wbScript, //[SCPT]
    wbString(SNAM, 'Sound') //[SOUN]
  ]).SetFormIDBase($40);

  wbRecord(LOCK, 'Lockpick', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbStruct(LKDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1.0, 2),
      wbInteger('Value', itS32),
      wbFloat('Quality', cpNormal, False, 1.0, 2),
      wbInteger('Uses', itS32)
    ]).SetRequired,
    wbScript, //[SCPT]
    wbIcon
  ]).SetFormIDBase($40);

  wbRecord(LTEX, 'Landscape Texture', [
    wbDeleted,
    wbEditorID,
    wbInteger(INTV, 'Texture ID', itU32),
    wbString(DATA, 'Texture Filename')
  ]).SetFormIDBase($60);

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
      wbFloat('Base Cost', cpNormal, False, 1.0, 2),
      wbInteger('Flags', itU32,
        wbFlags([
        {0}  'Target Skill',
        {1}  'Target Attribute',
        {2}  'No Duration',
        {3}  'No Magnitude',
        {4}  'Harmful',
        {5}  'Continuous VFX',
        {6}  'Cast Self',
        {7}  'Cast Touch',
        {8}  'Cast Target',
        {9}  'Spellmaking',
        {10} 'Enchanting',
        {11} 'Negative',
        {12} 'Applied Once',
        {13} 'Stealth',
        {14} 'Non-Recastable',
        {15} 'Illegal Daedra',
        {16} 'Non-reflectable',
        {17} 'Caster Linked'
        ])).IncludeFlag(dfCollapsed, wbCollapseFlags),
      wbStruct('Color', [
        wbInteger('Red', itU32),
        wbInteger('Green', itU32),
        wbInteger('Blue', itU32)
      ]).SetToStr(wbRGBAToStr)
        .IncludeFlag(dfCollapsed, wbCollapseRGBA),
      wbFloat('Size Multiplier', cpNormal, False, 1.0, 2),
      wbFloat('Speed Multiplier', cpNormal, False, 1.0, 2),
      wbFloat('Size Cap', cpNormal, False, 1.0, 2)
    ]).SetRequired,
    wbString(ITEX, 'Effect Texture Filename'),
    wbString(PTEX, 'Particle Texture Filename'),
    wbString(BSND, 'Bolt Sound'), //[SOUN]
    wbString(CSND, 'Cast Sound'), //[SOUN]
    wbString(HSND, 'Hit Sound'), //[SOUN]
    wbString(ASND, 'Area Sound'), //[SOUN]
    wbString(CVFX, 'Casting Visual'), //[STAT]
    wbString(BVFX, 'Bolt Visual'), //[WEAP]
    wbString(HVFX, 'Hit Visual'), //[STAT]
    wbString(AVFX, 'Area Visual'), //[STAT]
    wbDescription
  ]).SetFormIDBase($02);

  wbRecord(MISC, 'Miscellaneous Item', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbStruct(MCDT,'Data', [
      wbFloat('Weight', cpNormal, False, 1.0, 2),
      wbInteger('Value', itS32),
      //This bool is only set true if the object is used in a KNAM on a REFR.
      wbInteger('Is Key', itU32, wbBoolEnum)
    ]).SetRequired,
    wbScript, //[SCPT]
    wbIcon
  ]).SetFormIDBase($40);

  wbRecord(NPC_, 'Non-Player Character', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbString(RNAM, 'Race'), //[RACE]
    wbString(CNAM, 'Class'), //[CLAS]
    wbString(ANAM, 'Faction'), //[FACT]
    wbString(BNAM, 'Head Body Part'), //[BODY]
    wbString(KNAM, 'Hair Body Part'), //[BODY]
    wbScript, //[SCPT]
    wbStruct(NPDT, 'Data', [
      wbUnion('Calculated Format', wbNPCDataDecider, [
        wbStruct('Non-Auto', [
          wbInteger('Level', itS16),
          wbStruct('Attributes', [
            wbInteger('Strength', itU8),
            wbInteger('Intelligence', itU8),
            wbInteger('Willpower', itU8),
            wbInteger('Agility', itU8),
            wbInteger('Speed', itU8),
            wbInteger('Endurance', itU8),
            wbInteger('Personality', itU8),
            wbInteger('Luck', itU8)
          ]),
          wbStruct('Skills', [
            wbInteger('Block', itU8),
            wbInteger('Armorer', itU8),
            wbInteger('Medium Armor', itU8),
            wbInteger('Heavy Armor', itU8),
            wbInteger('Blunt Weapon', itU8),
            wbInteger('Long Blade', itU8),
            wbInteger('Axe', itU8),
            wbInteger('Spear', itU8),
            wbInteger('Athletics', itU8),
            wbInteger('Enchant', itU8),
            wbInteger('Destruction', itU8),
            wbInteger('Alteration', itU8),
            wbInteger('Illusion', itU8),
            wbInteger('Conjuration', itU8),
            wbInteger('Mysticism', itU8),
            wbInteger('Restoration', itU8),
            wbInteger('Alchemy', itU8),
            wbInteger('Unarmored', itU8),
            wbInteger('Security', itU8),
            wbInteger('Sneak', itU8),
            wbInteger('Acrobatics', itU8),
            wbInteger('Light Armor', itU8),
            wbInteger('Short Blade', itU8),
            wbInteger('Marksman', itU8),
            wbInteger('Speechcraft', itU8),
            wbInteger('Mercantile', itU8),
            wbInteger('Hand-to-Hand', itU8)
          ]),
          wbUnused(1),
          wbInteger('Health', itU16),
          wbInteger('Magicka', itU16),
          wbInteger('Fatigue', itU16),
          wbInteger('Disposition', itU8),
          wbInteger('Reputation', itU8),
          wbInteger('Rank', itU8),
          wbUnused(1),
          wbInteger('Gold', itU32)
        ]),
        wbStruct('Auto', [
          wbInteger('Level', itU16),
          wbInteger('Disposition', itU8),
          wbInteger('Reputation', itU8),
          wbInteger('Rank', itU8),
          wbUnused(3),
          wbInteger('Gold', itU32)
        ])
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
      ], False, 12))),
    wbRArray('Item Entries',
      wbStruct(NPCO, 'Item Entry', [
        wbInteger('Count', itS32),
        wbString('Item', 32) //[ALCH, APPA, ARMO, BOOK, CLOT, INGR, LEVI, LIGH, LOCK, MISC, PROB, REPA, WEAP]
      ])),
    wbRArray('Spells', wbString(NPCS, 'Spell', 32)), //[SPEL]
    wbAIData,
    wbTravelServices,
    wbAIPackages,
    wbFloat(XSCL, 'Scale', cpNormal, False, 1.0, 2)
  ]).SetFormIDBase($40);

  wbRecord(PGRD, 'Path Grid', [
    wbStruct(DATA, 'Data', [
      wbStruct('Grid', [
        wbInteger('X', itS32),
        wbInteger('Y', itS32)
      ], cpCritical),
      wbInteger('Granularity', itU16),
      wbInteger('Grid Point Count', itU16)
    ]).SetRequired,
    wbString(NAME, 'Location ID', 0, cpIgnore),
    IfThen(wbSimpleRecords,
      wbArray(PGRP, 'Grid Points',
        wbByteArray('Grid Point', 16)
      ).SetCountPathOnValue('DATA\Grid Point Count', False),
      wbArray(PGRP, 'Grid Points',
        wbStruct('Grid Point', [
          wbStruct('Position', [
            wbInteger('X', itS32),
            wbInteger('Y', itS32),
            wbInteger('Z', itS32)
          ]),
          wbInteger('User Created Point', itU8, wbBoolEnum),
          wbInteger('Number of Connections', itU8),
          wbUnused(2)
        ])
      ).SetCountPathOnValue('DATA\Grid Point Count', False)),
    IfThen(wbSimpleRecords,
      wbByteArray(PGRC, 'Grid Point Connections'),
      wbArray(PGRC, 'Grid Point Connections',
        wbArrayS('Grid Point Connection',
          wbInteger('Point', itU32),
        wbCalcPGRCSize)))
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
      Result := aMainRecord.GetGridCell(GridCell) and GridCellToFormID($E0, GridCell, aFormID);
    end)
    .SetIdentityCallback(function(const aMainRecord: IwbMainRecord): string begin
      var GridCell: TwbGridCell;
      if aMainRecord.GetGridCell(GridCell) then
        Result := '<Exterior>' + GridCell.SortKey
      else
        Result := aMainRecord.EditorID;
    end);

  wbRecord(PROB, 'Probe', [
    wbEditorID,
    wbDeleted,
    wbModel,
    wbFullName,
    wbStruct(PBDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1.0, 2),
      wbInteger('Value', itS32),
      wbFloat('Quality', cpNormal, False, 1.0, 2),
      wbInteger('Uses', itS32)
    ]).SetRequired,
    wbScript, //[SCPT]
    wbIcon
  ]).SetFormIDBase($40);

  wbRecord(RACE, 'Race', [
    wbEditorID,
    wbDeleted,
    wbFullName,
    wbStruct(RADT, 'Data', [
      wbArray('Skill Bonuses',
        wbStruct('Skill Bonus', [
          wbInteger('Skill', itS32, wbSkillEnum),
          wbInteger('Bonus', itS32)
        ]),
      7),
      wbStruct('Base Attributes', [
        wbStruct('Strength', [
          wbInteger('Male', itS32),
          wbInteger('Female', itS32)
        ]),
        wbStruct('Intelligence', [
          wbInteger('Male', itS32),
          wbInteger('Female', itS32)
        ]),
        wbStruct('Willpower', [
          wbInteger('Male', itS32),
          wbInteger('Female', itS32)
        ]),
        wbStruct('Agility', [
          wbInteger('Male', itS32),
          wbInteger('Female', itS32)
        ]),
        wbStruct('Speed', [
          wbInteger('Male', itS32),
          wbInteger('Female', itS32)
        ]),
        wbStruct('Endurance', [
          wbInteger('Male', itS32),
          wbInteger('Female', itS32)
        ]),
        wbStruct('Personality', [
          wbInteger('Male', itS32),
          wbInteger('Female', itS32)
        ]),
        wbStruct('Luck', [
          wbInteger('Male', itS32),
          wbInteger('Female', itS32)
        ])
      ]),
      wbStruct('Height', [
        wbFloat('Male', cpNormal, False, 1.0, 2),
        wbFloat('Female', cpNormal, False, 1.0, 2)
      ]),
      wbStruct('Weight', [
        wbFloat('Male', cpNormal, False, 1.0, 2),
        wbFloat('Female', cpNormal, False, 1.0, 2)
      ]),
      wbInteger('Flags', itU32,
        wbFlags([
        {0} 'Playable',
        {1} 'Beast Race'
        ])).IncludeFlag(dfCollapsed, wbCollapseFlags)
    ]).SetRequired,
    wbRArray('Spells', wbStringForward(NPCS, 'Spell', 32)),
    wbDescription
  ]).SetFormIDBase($14);

  wbRecord(REFR, 'Reference', @wbKnownSubRecordSignaturesREFR, [
    wbStruct(CNDT, 'Previous Cell Grid', [
      wbInteger('X', itS32),
      wbInteger('Y', itS32)
    ]),
    wbInteger(FRMR, 'Object Index', itU32, wbFRMRToString, nil, cpIgnore, True).IncludeFlag(dfInternalEditOnly),
    wbString(NAME, 'Base Object'), //[ACTI, ALCH, APPA, ARMO, BODY, BOOK, CLOT, CONT, CREA, DOOR, INGR, LEVC, LOCK, MISC, NPC_, PROB, REPA, STAT, WEAP]
    wbInteger(UNAM, 'Reference Blocked', itU8, wbEnum(['Blocked'])),
    wbFloat(XSCL, 'Scale', cpNormal, False, 1, 2),
    wbRStructSK([], 'Owner Data', [
      wbString(ANAM, 'Owner'), //[NPC_]
      wbString(BNAM, 'Global Variable'), //[GLOB]
      wbString(CNAM, 'Faction Owner'), //[FACT]
      wbInteger(INDX, 'Faction Rank', itS32)
    ], [], cpNormal, False, nil, True),
    wbFloat(XCHG, 'Enchanting Charge', cpNormal, False, 1, 0),
    wbString(XSOL, 'Soul'), //[CREA]
    wbInteger(INTV, 'Health', itS32),
    wbInteger(NAM9, 'Count', itS32),
    wbRStructSK([], 'Teleport Data', [
      wbStruct(DODT, 'Teleport Destination', [
        wbVec3('Position'),
        wbVec3('Rotation')
      ]),
      wbString(DNAM, 'Teleport Cell') //[CELL]
    ]),
    wbRStructSK([], 'Lock Data', [
      wbInteger(FLTV, 'Lock Level', itU32),
      wbString(KNAM, 'Key'), //[MISC]
      wbString(TNAM, 'Trap') //[ENCH]
    ], [], cpNormal, False, nil, True),
    wbInteger(DELE, 'Deleted', itU32,
      wbEnum([], [
        $00482C64, 'Deleted',
        $11842014, 'Deleted (Door Reference)'
    ])).SetDefaultNativeValue(4729956),
    wbStruct(DATA, 'Reference Data', [
      wbVec3('Position'),
      wbVec3('Rotation')
    ])
  ]).SetGetFormIDCallback(function(const aMainRecord: IwbMainRecord; out aFormID: TwbFormID): Boolean begin
      var lFRMR := aMainRecord.RecordBySignature[FRMR];
      Result := Assigned(lFRMR);
      if Result then begin
        aFormID := TwbFormID.FromCardinal(lFRMR.NativeValue);
        if aFormID.FileID.FullSlot = 0 then
          aFormID.FileID := TwbFileID.CreateFull($FF);
      end;
    end);

  wbRecord(REGN, 'Region', [
    wbDeleted,
    wbEditorID.SetRequired,
    wbFullName.SetRequired,
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
    ], cpNormal, True, nil, 8),
    wbString(BNAM, 'Sleep Creature'), //[LEVC]
    wbByteColors(CNAM, 'Map Color').SetRequired,
    wbRArray('Sound Records',
      wbStruct(SNAM, 'Sound Record', [
        wbString(True, 'Sound', 32), //[SOUN]
        wbInteger('Chance', itU8)
      ]))
  ]).SetFormIDBase($70);

  wbRecord(REPA, 'Repair Item', [
    wbEditorID.SetRequired,
    wbDeleted,
    wbModel.SetRequired,
    wbFullName,
    wbStruct(RIDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1),
      wbInteger('Uses', itU32).SetDefaultNativeValue(10),
      wbFloat('Quality', cpNormal, False, 1, 2).SetDefaultNativeValue(1)
    ]).SetRequired,
    wbScript, //[SCPT]
    wbIcon
  ]).SetFormIDBase($40);

  wbRecord(SCPT, 'Script', @wbKnownSubRecordSignaturesSCPT, [
    wbStruct(SCHD, 'Script Header', [
      //Name can be saved with 36 characters in the CS, but it collides with Number of Shorts.
      wbString('Name', 32),
      wbInteger('Number of Shorts', itU32),
      wbInteger('Number of Longs', itU32),
      wbInteger('Number of Floats', itU32),
      wbInteger('Script Data Size', itU32),
      wbInteger('Local Variable Size', itU32)
    ]).SetRequired,
    wbDeleted,
    wbArray(SCVR, 'Script Variables', wbString('Script Variable', 0, cpCritical)),
    wbByteArray(SCDT, 'Compiled Script'),
    wbStringScript(SCTX, 'Script Source').SetRequired
  ]).SetFormIDBase($30)
    .SetGetEditorIDCallback(function (const aSubRecord: IwbSubRecord): string begin
      Result := aSubRecord.ElementEditValues['Name'];
    end)
    .SetSetEditorIDCallback(procedure (const aSubRecord: IwbSubRecord; const aEditorID: string) begin
      aSubRecord.ElementEditValues['Name'] := aEditorID;
    end);

  wbRecord(SKIL, 'Skill', @wbKnownSubRecordSignaturesINDX, [
    wbInteger(INDX, 'Name', itU32, wbSkillEnum),
    wbStruct(SKDT, 'Data', [
      wbInteger('Attribute', itS32, wbAttributeEnum),
      wbInteger('Type', itU32, wbSpecializationEnum),
      wbArray('Actions', wbFloat('Action'), 4)
    ]).SetRequired,
    wbDescription
  ]).SetFormIDBase($01);

  wbRecord(SNDG, 'Sound Generator', [
    wbEditorID.SetRequired,
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
      ])),
    wbString(CNAM, 'Creature'), //[CREA]
    wbString(SNAM, 'Sound'), //[SOUN]
    wbDeleted
  ]).SetFormIDBase($28);

  wbRecord(SOUN, 'Sound', @wbKnownSubRecordSignaturesNoFNAM, [
    wbEditorID.SetRequired,
    wbDeleted,
    wbString(FNAM, 'Sound Filename').SetRequired,
    wbStruct(DATA, 'Data', [
      wbInteger('Volume', itU8).SetDefaultNativeValue(1),
      wbInteger('Minimum Range', itU8),
      wbInteger('Maximum Range', itU8)
    ]).SetRequired
  ]).SetFormIDBase($40);

  wbRecord(SPEL, 'Spellmaking', [
    wbEditorID.SetRequired,
    wbDeleted,
    wbFullName,
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
      wbInteger('Spell Cost', itS32),
      wbInteger('Flags', itU32,
        wbFlags([
        {0} 'Auto Calculate Cost',
        {1} 'PC Start Spell',
        {2} 'Always Succeeds'
        ])).IncludeFlag(dfCollapsed, wbCollapseFlags)
    ]).SetRequired,
    wbEffects
  ]).SetFormIDBase($0A);

  wbRecord(SSCR, 'Start Script', @wbKnownSubRecordSignaturesSSCR, [
    wbInteger(DELE, 'Deleted', itU32,
      wbEnum([],[1, 'Deleted'])
    ).SetDefaultNativeValue(1),
    wbString(DATA, 'Numerical ID'),
    wbString(NAME, 'Script') //[SCPT]
  ]).SetFormIDBase($3F);

  wbRecord(STAT, 'Static', [
    wbEditorID.SetRequired,
    wbDeleted,
    wbModel.SetRequired
  ]).SetFormIDBase($40);

  wbRecord(WEAP, 'Weapon', [
    wbEditorID.SetRequired,
    wbDeleted,
    wbModel.SetRequired,
    wbFullName,
    wbStruct(WPDT, 'Data', [
      wbFloat('Weight', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Value', itU32).SetDefaultNativeValue(1),
      wbInteger('Type', itU16,
        wbEnum([
        {0}  'Short Blade One Hand',
        {1}  'Long Blade One Hand',
        {2}  'Long Blade Two Close',
        {3}  'Blunt One Hand',
        {4}  'Blunt Two Close',
        {5}  'Blunt Two Wide',
        {6}  'Spear Two Wide',
        {7}  'Axe One Hand',
        {8}  'Axe Two Hand',
        {9}  'Marksman Bow',
        {10} 'Marksman Crossbow',
        {11} 'Marksman Thrown',
        {12} 'Arrow',
        {13} 'Bolt'
        ])).SetDefaultNativeValue(12),
      wbInteger('Health', itU16).SetDefaultNativeValue(100),
      wbFloat('Speed', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbFloat('Reach', cpNormal, False, 1, 2).SetDefaultNativeValue(1),
      wbInteger('Enchanting Charge', itU16).SetDefaultNativeValue(100),
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
    wbScript, //[SCPT]
    wbIcon,
    wbEnchantment //[ENCH]
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
