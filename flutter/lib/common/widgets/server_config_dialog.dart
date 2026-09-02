import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
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

    Future<void> _move(String id, int newIndex) async {
      final err = await state.move(id, newIndex);
      if (err == null) {
        refresh();
        showToast(translate('Successful'));
      } else {
        showToast(err);
        refresh();
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
      // Raises the cap the dialog framework puts on content width; its default
      // of 500 would otherwise clamp the content below its intended width.
      contentBoxConstraints: const BoxConstraints(maxWidth: 575),
      content: ConstrainedBox(
        // Keeps the whole dialog under 850 high: the title and the actions bar
        // take about 176 of it, so the content is capped at 670 and the list
        // scrolls instead of stretching the dialog.
        constraints: const BoxConstraints(maxHeight: 670),
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
                child: ReorderableListView.builder(
                  shrinkWrap: true,
                  buildDefaultDragHandles: false,
                  itemCount: state.configs.length,
                  onReorder: (oldIndex, newIndex) {
                    // Index 0 is the default and is pinned, so it must never move.
                    if (oldIndex <= 0 || newIndex <= 0) return;
                    var target = newIndex;
                    // ReorderableListView reports the target index after the removed item
                    // would have shifted the tail, so pulling an item down needs a -1.
                    if (target > oldIndex) target -= 1;
                    final item = state.configs[oldIndex];
                    _move(item.id, target);
                  },
                  itemBuilder: (context, index) {
                    final item = state.configs[index];
                    final card = ServerConfigCard(
                      config: item,
                      isCurrent: item.isCurrent,
                      onSwitch: () async {
                        final oldApiServer = await bind.mainGetApiServer();
                        final err = await state.switchTo(item.id);
                        final newApiServer = await bind.mainGetApiServer();
                        if (err == null) {
                          if (oldApiServer.isNotEmpty &&
                              oldApiServer != newApiServer &&
                              gFFI.userModel.isLogin) {
                            gFFI.userModel.logOut(apiServer: oldApiServer);
                          }
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
                      onMoveUp: index <= 1
                          ? null
                          : () => _move(item.id, index - 1),
                      onMoveDown: index >= state.configs.length - 1
                          ? null
                          : () => _move(item.id, index + 1),
                    );
                    // The default item is not wrapped in a drag listener, so it stays put
                    // and the user cannot drag other items above it.
                    return Container(
                      key: Key(item.id),
                      child: item.isDefault
                          ? card
                          : ReorderableDragStartListener(
                              index: index,
                              child: card,
                            ),
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
