import type { ReactNode } from "react";
import type { EnumEntry, ParseXmlResponse } from "../../xml_model/uigraph";
import type { NodeValueEntry, SfncGroup } from "../../device/types";
import { useSfncGroups } from "../../device/useSfncGroups";
import { isSectionApplicable } from "./sfncGroupUtils";
import { SidebarSection } from "./SidebarSection";
import { useSectionPersistence } from "./useSectionPersistence";
import { AcquisitionSection } from "./AcquisitionSection";
import { ExposureGainSection } from "./ExposureGainSection";
import { ImageFormatSection } from "./ImageFormatSection";
import { TriggerSection } from "./TriggerSection";
import { TransportLayerSection } from "./TransportLayerSection";
import { AutoFunctionsSection } from "./AutoFunctionsSection";
import { ColorProcessingSection } from "./ColorProcessingSection";

interface ControlSidebarProps {
  collapsed: boolean;
  onToggle: () => void;
  isConnected: boolean;
  isAcquiring: boolean;
  frameCount: number;
  onStartAcq: () => Promise<void>;
  onStopAcq: () => Promise<void>;
  acquisitionModeEntries: EnumEntry[];
  liveValues: Map<string, NodeValueEntry>;
  externalModel: ParseXmlResponse | null;
  cameraModel: string | null;
}

export function ControlSidebar({
  collapsed,
  onToggle,
  isConnected,
  isAcquiring,
  frameCount,
  onStartAcq,
  onStopAcq,
  acquisitionModeEntries,
  liveValues,
  externalModel,
  cameraModel,
}: ControlSidebarProps) {
  const groups = useSfncGroups();
  const sectionPersistence = useSectionPersistence(cameraModel);
  const nodesById = externalModel?.graph.nodes_by_name ?? {};
  const visibleGroups = groups.filter((g) => isSectionApplicable(g, nodesById));

  function renderSectionContent(group: SfncGroup): ReactNode {
    switch (group.id) {
      case "acquisition_control":
        return (
          <AcquisitionSection
            isConnected={isConnected}
            isAcquiring={isAcquiring}
            frameCount={frameCount}
            onStartAcq={onStartAcq}
            onStopAcq={onStopAcq}
            acquisitionModeEntries={acquisitionModeEntries}
          />
        );
      case "exposure_gain":
        return (
          <ExposureGainSection
            isConnected={isConnected}
            externalModel={externalModel}
            liveValues={liveValues}
          />
        );
      case "image_format":
        return (
          <ImageFormatSection
            isConnected={isConnected}
            externalModel={externalModel}
            liveValues={liveValues}
          />
        );
      case "trigger":
        return (
          <TriggerSection
            isConnected={isConnected}
            externalModel={externalModel}
            liveValues={liveValues}
          />
        );
      case "transport_layer":
        return (
          <TransportLayerSection
            isConnected={isConnected}
            externalModel={externalModel}
            liveValues={liveValues}
          />
        );
      case "auto_functions":
        return (
          <AutoFunctionsSection
            isConnected={isConnected}
            externalModel={externalModel}
            liveValues={liveValues}
          />
        );
      case "color_processing":
        return (
          <ColorProcessingSection
            isConnected={isConnected}
            externalModel={externalModel}
            liveValues={liveValues}
          />
        );
      default:
        return <p className="sidebar-placeholder">Available in a future update.</p>;
    }
  }

  return (
    <aside
      className={`iv-sidebar${collapsed ? " iv-sidebar--collapsed" : ""}`}
      aria-label="Control sidebar"
    >
      <button
        type="button"
        className={`iv-sidebar__toggle${collapsed ? " iv-sidebar__toggle--collapsed" : ""}`}
        aria-label={collapsed ? "Show controls" : "Hide controls"}
        title={collapsed ? "Show controls" : "Hide controls"}
        onClick={onToggle}
      >
        <span className="iv-sidebar__toggle-arrow" />
        {!collapsed && <span className="iv-sidebar__toggle-label">Controls</span>}
      </button>

      {collapsed ? (
        <div className="iv-sidebar__rail">
          {visibleGroups.map((group) => (
            <button
              key={group.id}
              type="button"
              className="iv-sidebar__rail-icon"
              title={group.title}
              onClick={onToggle}
            >
              {group.icon}
            </button>
          ))}
        </div>
      ) : (
        <>
          {groups.length === 0 ? (
            <p className="sidebar-placeholder">Loading controls…</p>
          ) : visibleGroups.length === 0 ? (
            <p className="sidebar-placeholder">No applicable sections for this device.</p>
          ) : (
            visibleGroups.map((group) => (
              <SidebarSection
                key={group.id}
                title={group.title}
                icon={group.icon}
                defaultOpen={sectionPersistence.isOpen(group.id, group.default_open)}
                onToggle={(open) => sectionPersistence.setOpen(group.id, open)}
              >
                {renderSectionContent(group)}
              </SidebarSection>
            ))
          )}
        </>
      )}
    </aside>
  );
}
