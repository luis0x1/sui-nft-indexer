-- AlterTable
ALTER TABLE "objects" ADD COLUMN     "display" JSONB NOT NULL DEFAULT '{}',
ADD COLUMN     "last_tx_digist" VARCHAR(66),
ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- CreateTable
CREATE TABLE "packages" (
    "id" VARCHAR(66) NOT NULL,
    "serialized" TEXT NOT NULL,
    "version" BIGINT NOT NULL,
    "last_tx_digist" VARCHAR(66) NOT NULL,
    "updated_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),

    CONSTRAINT "packages_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "displays" (
    "id" VARCHAR(66) NOT NULL,
    "fields" JSONB NOT NULL,
    "last_tx_digist" VARCHAR(66) NOT NULL,
    "updated_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),

    CONSTRAINT "displays_pkey" PRIMARY KEY ("id")
);
