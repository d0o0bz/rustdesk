import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/server_config_model.dart';

/// 状态徽标：可用(绿) / 不可用(红) / 未检测(灰)。
class ServerStatusBadge extends StatelessWidget {
  final bool? isAvailable;

  const ServerStatusBadge({Key? key, this.isAvailable}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final Color color;
    final String label;
    if (isAvailable == null) {
      color = MyTheme.darkGray;
      label = translate('Not checked');
    } else if (isAvailable!) {
      color = const Color(0xFF32bea6);
      label = translate('Available');
    } else {
      color = const Color(0xFFE04F5F);
      label = translate('Unavailable');
    }
    return Row(mainAxisSize: MainAxisSize.min, children: [
      Container(
        width: 8,
        height: 8,
        decoration: BoxDecoration(color: color, shape: BoxShape.circle),
      ),
      const SizedBox(width: 4),
      Text(label, style: const TextStyle(fontSize: 12, color: Colors.grey)),
    ]);
  }
}

/// 单个服务器配置卡片，展示名称、地址、端口、徽标与延迟，并提供操作按钮。
class ServerConfigCard extends StatelessWidget {
  final ServerConfigItem config;
  final bool isCurrent;
  final VoidCallback onSwitch;
  final VoidCallback onEdit;
  final VoidCallback onDelete;
  final VoidCallback onCheck;
  final VoidCallback onSetDefault;

  const ServerConfigCard({
    Key? key,
    required this.config,
    required this.isCurrent,
    required this.onSwitch,
    required this.onEdit,
    required this.onDelete,
    required this.onCheck,
    required this.onSetDefault,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final address = config.relayServer.isEmpty
        ? '${config.idServer}:${config.idPort}'
        : '${config.idServer}:${config.idPort} / ${config.relayServer}:${config.relayPort ?? ''}';

    return Card(
      margin: const EdgeInsets.symmetric(vertical: 6),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Row(
                    children: [
                      Flexible(
                        child: Text(
                          config.name,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontWeight: FontWeight.w600, fontSize: 15),
                        ),
                      ),
                      const SizedBox(width: 8),
                      if (config.isDefault)
                        _Tag(label: translate('Default')),
                      if (isCurrent) _Tag(label: translate('In use'), accent: true),
                    ],
                  ),
                ),
                if (isCurrent)
                  Icon(Icons.check_circle,
                      color: MyTheme.accent, size: 18)
                else
                  TextButton.icon(
                    icon: const Icon(Icons.swap_horiz, size: 18),
                    label: Text(translate('Use')),
                    onPressed: onSwitch,
                  ),
              ],
            ),
            const SizedBox(height: 6),
            Text(
              address,
              style: const TextStyle(fontSize: 13, color: Colors.grey),
              overflow: TextOverflow.ellipsis,
            ),
            if (config.apiServer.isNotEmpty) ...[
              const SizedBox(height: 2),
              Text(
                '${translate('API Server')}: ${config.apiServer}',
                style: const TextStyle(fontSize: 13, color: Colors.grey),
                overflow: TextOverflow.ellipsis,
              ),
            ],
            const SizedBox(height: 8),
            Row(
              children: [
                ServerStatusBadge(isAvailable: config.isAvailable),
                if (config.avgLatency != null) ...[
                  const SizedBox(width: 12),
                  Text(
                    '${translate('Latency')}: ${config.avgLatency}ms',
                    style: const TextStyle(fontSize: 12, color: Colors.grey),
                  ),
                ],
                const Spacer(),
                if (!config.isDefault)
                  TextButton.icon(
                    icon: const Icon(Icons.star_outline, size: 18),
                    label: Text(translate('Set as default')),
                    onPressed: onSetDefault,
                  ),
                IconButton(
                  icon: const Icon(Icons.network_check, size: 18),
                  tooltip: translate('Check availability'),
                  onPressed: onCheck,
                ),
                IconButton(
                  icon: const Icon(Icons.edit_outlined, size: 18),
                  tooltip: translate('Edit'),
                  onPressed: onEdit,
                ),
                if (!config.isDefault)
                  IconButton(
                    icon: const Icon(Icons.delete_outline, size: 18),
                    tooltip: translate('Delete'),
                    onPressed: onDelete,
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _Tag extends StatelessWidget {
  final String label;
  final bool accent;

  const _Tag({Key? key, required this.label, this.accent = false})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: accent ? MyTheme.accent50 : MyTheme.darkGray.withOpacity(0.2),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 11,
          color: accent ? MyTheme.accent : Colors.grey,
        ),
      ),
    );
  }
}
