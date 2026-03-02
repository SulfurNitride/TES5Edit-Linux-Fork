{******************************************************************************

  xEdit Lazarus/LCL Port
  Cross-platform GUI for xEdit (Linux + Windows)

  This Source Code Form is subject to the terms of the Mozilla Public License,
  v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain
  one at https://mozilla.org/MPL/2.0/.

*******************************************************************************}

program xEditLaz;

{$mode objfpc}{$H+}

uses
  {$IFDEF UNIX}
  cthreads,
  cmem,
  {$ENDIF}
  Interfaces, // this includes the LCL widgetset
  Forms,
  SysUtils,
  xeMainFormLaz;

begin
  RequireDerivedFormResource := True;
  Application.Scaled := True;
  Application.Initialize;

  SysUtils.FormatSettings.DecimalSeparator := '.';

  Application.Title := 'xEdit (Lazarus Port)';
  Application.CreateForm(TfrmMain, frmMain);
  Application.Run;
end.
