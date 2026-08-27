import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/server_config_model.dart';

import 'server_config_widgets.dart';

/// 多服务器配置管理弹窗，桌面端与移动端共用。
Future<void> showServerConfigManager(
  OverlayDialogManager dialogManager,
) async {
  final state = ServerConfigState();
  await state.load();

  dialogManager.show((setState, close, context) {
    void refresh() => setState(() {});

    Future<void> _showEditDialog([ServerConfigItem? item]) async {
      final isEdit = item != null;
      final nameCtrl = TextEditingController(text: item?.name ?? '');
      final idServerCtrl = TextEditingController(text: item?.idServer ?? '');
      final idPortCtrl =
          TextEditingController(text: item != null ? '${item.idPort}' : '');
      final relayServerCtrl =
          TextEditingController(text: item?.relayServer ?? '');
      final relayPortCtrl = TextEditingController(
          text: item?.relayPort != null ? '${item?.relayPort}' : '');
      final apiServerCtrl =
          TextEditingController(text: item?.apiServer ?? '');
      final keyCtrl = TextEditingController(text: item?.key ?? '');
      var inProgress = false;

      await dialogManager.show<bool>((editSetState, editClose, _) {
        Future<void> submit() async {
          editSetState(() => inProgress = true);
          final name = nameCtrl.text.trim();
          final idServer = idServerCtrl.text.trim();
          final idPort = int.tryParse(idPortCtrl.text.trim()) ?? 21116;
          final relayServer = relayServerCtrl.text.trim();
          final relayPort = int.tryParse(relayPortCtrl.text.trim()) ?? 21117;
          final apiServer = apiServerCtrl.text.trim();
          final key = keyCtrl.text.trim();

          String? err;
          if (isEdit) {
            err = await state.update(
              id: item!.id,
              name: name,
              idServer: idServer,
              idPort: idPort,
              relayServer: relayServer,
              relayPort: relayPort,
              apiServer: apiServer,
              key: key,
            );
          } else {
            err = await state.add(
              name: name,
              idServer: idServer,
              idPort: idPort,
              relayServer: relayServer,
              relayPort: relayPort,
              apiServer: apiServer,
              key: key,
            );
          }
          editSetState(() => inProgress = false);
          if (err == null) {
            editClose(true);
            refresh();
            showToast(translate('Successful'));
          } else {
            showToast(err);
          }
        }

        Widget field(String label, TextEditingController c,
            {bool numeric = false, bool autofocus = false}) {
          return TextFormField(
            controller: c,
            autofocus: autofocus,
            decoration: InputDecoration(labelText: label),
            keyboardType:
                numeric ? TextInputType.number : TextInputType.visiblePassword,
            textCapitalization: TextCapitalization.none,
            autocorrect: false,
            enableSuggestions: false,
            smartDashesType: SmartDashesType.disabled,
            smartQuotesType: SmartQuotesType.disabled,
            enableIMEPersonalizedLearning: false,
            spellCheckConfiguration:
                const SpellCheckConfiguration.disabled(),
            validator: (v) {
              if ((label == translate('Name') ||
                      label == translate('ID Server')) &&
                  (v == null || v.trim().isEmpty)) {
                return translate('Required');
              }
              return null;
            },
          );
        }

        return CustomAlertDialog(
          title: Text(translate(isEdit ? 'Edit server config' : 'Add server config')),
          content: ConstrainedBox(
            constraints: const BoxConstraints(minWidth: 400),
            child: Form(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  field(translate('Name'), nameCtrl, autofocus: true),
                  const SizedBox(height: 8),
                  field(translate('ID Server'), idServerCtrl),
                  const SizedBox(height: 8),
                  field(translate('ID Server Port'), idPortCtrl, numeric: true),
                  const SizedBox(height: 8),
                  if (!isIOS && !isWeb) ...[
                    field(translate('Relay Server'), relayServerCtrl),
                    const SizedBox(height: 8),
                    field(translate('Relay Server Port'), relayPortCtrl,
                        numeric: true),
                  ],
                  if (!isIOS && !isWeb) ...[
                    field(translate('API Server'), apiServerCtrl),
                    const SizedBox(height: 8),
                    field(translate('Key'), keyCtrl),
                  ],
                  if (inProgress)
                    const Padding(
                      padding: EdgeInsets.only(top: 8),
                      child: LinearProgressIndicator(),
                    ),
                ],
              ),
            ),
          ),
          actions: [
            dialogButton('Cancel',
                onPressed: () => editClose(false), isOutline: true),
            dialogButton('OK', onPressed: submit),
          ],
          onSubmit: submit,
          onCancel: () => editClose(false),
        );
      });
    }

    Future<void> _delete(ServerConfigItem item) async {
      final confirmed = await dialogManager.show<bool>((_, dClose, __) {
        return CustomAlertDialog(
          title: Text(translate('Delete server config')),
          content: Text(translate('Delete server config tip')
              .replaceAll('{name}', item.name)),
          actions: [
            dialogButton('Cancel',
                onPressed: () => dClose(false), isOutline: true),
            dialogButton('OK', onPressed: () => dClose(true)),
          ],
          onSubmit: () => dClose(true),
          onCancel: () => dClose(false),
        );
      });
      if (confirmed == true) {
        final err = await state.delete(item.id);
        if (err == null) {
          refresh();
          showToast(translate('Successful'));
        } else {
          showToast(err);
        }
      }
    }

    return CustomAlertDialog(
      title: Row(
        children: [
          Expanded(child: Text(translate('Multiple server config'))),
          IconButton(
            icon: const Icon(Icons.add),
            tooltip: translate('Add server config'),
            onPressed: () => _showEditDialog(),
          ),
        ],
      ),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(translate('Auto switch server')),
              value: state.autoSwitchEnabled,
              onChanged: (v) async {
                await state.setAutoSwitch(v);
                refresh();
              },
            ),
            const Divider(height: 1),
            if (state.loading)
              const Padding(
                padding: EdgeInsets.all(16),
                child: Center(child: CircularProgressIndicator()),
              )
            else if (state.configs.isEmpty)
              Padding(
                padding: const EdgeInsets.all(16),
                child: Center(
                    child: Text(translate('No server config'))),
              )
            else
              Flexible(
                child: ListView.builder(
                  shrinkWrap: true,
                  itemCount: state.configs.length,
                  itemBuilder: (context, index) {
                    final item = state.configs[index];
                    return ServerConfigCard(
                      config: item,
                      isCurrent: item.isCurrent,
                      onSwitch: () async {
                        final err = await state.switchTo(item.id);
                        if (err == null) {
                          refresh();
                          showToast(translate('Successful'));
                        } else {
                          showToast(err);
                        }
                      },
                      onEdit: () => _showEditDialog(item),
                      onDelete: () => _delete(item),
                      onCheck: () async {
                        final result = await state.check(item.id);
                        refresh();
                        showToast(result == null
                            ? translate('Failed')
                            : translate('Successful'));
                      },
                    );
                  },
                ),
              ),
          ],
        ),
      ),
      actions: [
        dialogButton('Close', onPressed: () => close(), isOutline: true),
      ],
      onCancel: () => close(),
    );
  }, backDismiss: true, clickMaskDismiss: true);
}
