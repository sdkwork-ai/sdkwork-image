import { ImageOff } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { CapabilityUnavailablePage } from "@sdkwork/ui-mobile-react";

export function AIImagePage() {
  const { t } = useTranslation("ai_image");
  const navigate = useNavigate();

  return (
    <CapabilityUnavailablePage
      icon={ImageOff}
      message={t("unavailable")}
      onBack={() => navigate(-1)}
      title={t("header_title")}
    />
  );
}
